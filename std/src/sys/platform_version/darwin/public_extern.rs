//! # 供其他编译器使用的运行时版本检查 ABI。
//!
//! 本文件中的这些符号对我们很有用，导出它们可以让那些使用以下语言版本检查功能的
//! 代码在链接时正常工作：
//! - Clang 的 `__builtin_available` 宏。
//! - Objective-C 的 `@available`。
//! - Swift 的 `#available`，
//!
//! 如果 Rust 不导出这些符号，用户在链接使用这些特性的
//! C/Objective-C/Swift 库时就会遇到链接错误。
//!
//! 这些符号的存在主要被视为一个实现质量（quality-of-implementation）细节，
//! 不应被依赖为始终可用。其意图在于让链接由 Clang 的 `__builtin_available`
//!（或类似机制）构建的代码能够继续工作。例如，如果不再支持 Clang 11（Xcode 11），
//! 我们可能会决定移除 `__isOSVersionAtLeast`。
//!
//! ## 背景
//!
//! 关于此特性的最初讨论可见于：
//! - <https://lists.llvm.org/pipermail/cfe-dev/2016-July/049851.html>
//! - <https://reviews.llvm.org/D27827>
//! - <https://reviews.llvm.org/D30136>
//!
//! 其上游实现可在 `compiler-rt` 中找到：
//! <https://github.com/llvm/llvm-project/blob/llvmorg-20.1.0/compiler-rt/lib/builtins/os_version_check.c>
//!
//! 理想情况下，这些符号大概本应是 Apple 的 `libSystem.dylib` 的一部分，原因之一是
//! 它们的实现相当复杂，用到了内存分配、环境变量、文件访问和动态库加载（并且会把这一切
//! 都塞进每一个二进制文件里）。
//!
//! Apple 当初为何选择不那样做已无从考证，但一个充分的理由可能是：将其作为 `compiler-rt`
//! 的一部分来实现，让他们能够立即把它向后部署（back-deploy）到较旧的操作系统上。
//!
//! 就 Rust 而言，虽然我们将来可能会提供一个类似 `@available` 的特性，但我们大概会以
//! `std` 导出的宏的形式来做（而不是作为编译器内建）。因此在 `std` 中实现这一点是合理的，
//! 既然如此我们就可以用 `std` 的工具来实现它，也可以避免让 `compiler-builtins` 依赖
//! `libSystem.dylib`。
//!
//! 这确实意味着，那些试图链接 C/Objective-C/Swift 代码_并且_在其所有 crate 中使用
//! `#![no_std]` 的用户，可能会因为这些符号缺失而得到链接错误。不过在 Apple 系统上使用
//! `no_std` 相当少见，因此不支持这种用例大概也无妨。
//!
//! 解决办法是链接 `libclang_rt.osx.a`，或以其他方式使用 Clang 的 `compiler-rt`。
//!
//! 另见 <https://github.com/rust-lang/compiler-builtins/pull/794> 中的讨论。
//!
//! ## 实现细节
//!
//! NOTE: 自 macOS 10.15 起，`libSystem.dylib` 实际上_确实_已通过 `libxpc` 提供了
//! 未公开的 `_availability_version_check` 来做版本查询（它是 zippered 的，这也是为何
//! 它需要一个 platform 参数来区分 macOS 与 Mac Catalyst），不过使用它可能有点危险，参见：
//! - <https://reviews.llvm.org/D150397>
//! - <https://github.com/llvm/llvm-project/issues/64227>
//!
//! 此外，为了支持较旧的版本，我们无论如何都需要通过 PList 来实现版本查询，因此干脆到处
//! 都用它（既然它在内联之后还能被进一步优化）。

#![allow(non_snake_case)]

use super::{current_version, pack_i32_os_version};

/// 当前平台的 OS 版本是否高于或等于给定的版本。
///
/// 第一个参数是调用方二进制的_基础_ Mach-O 平台（即 `PLATFORM_MACOS`、`PLATFORM_IOS`
/// 等，但不是 `PLATFORM_IOSSIMULATOR` 或 `PLATFORM_MACCATALYST`）。
///
/// 参数由 Clang 静态指定。配合 LTO 内联应当能让这些版本被合并成单个 `u32`，从而让比较
/// 更快，并让 `BASE_TARGET_PLATFORM` 检查成为空操作（no-op）。
//
// SAFETY: 此签名与 Clang 所期望的相同，并且我们以弱（weak）符号导出，以允许同时链接本实现
// 和 `libclang_rt.*.a`，与 `compiler-builtins` 的做法类似：
// https://github.com/rust-lang/compiler-builtins/blob/0.1.113/src/macros.rs#L494
//
// NOTE: 这个符号在编译器的符号名修饰（mangling）中有一个变通处理，以避免对其进行修饰，
// 同时又不会从非 cdylib 中暴露它（不像 `#[no_mangle]` 那样）。
#[rustc_std_internal_symbol]
// NOTE: 把它做成弱符号可能并不完全是正确的解决方案，`compiler_rt` 并不这么做，它转而让
// 该符号具有 "hidden" 可见性。但由于这里放在 `libstd` 中，而 `libstd` 可能被当作 dylib
// 使用，因此我们这里不能照搬那种做法。
#[linkage = "weak"]
// extern "C" 是正确的，Clang 假定该函数不会 unwind：
// https://github.com/llvm/llvm-project/blob/llvmorg-20.1.0/clang/lib/CodeGen/CGObjC.cpp#L3980
//
// 如果其中发生错误，我们转而 abort 整个进程。
pub(super) extern "C" fn __isPlatformVersionAtLeast(
    platform: i32,
    major: i32,
    minor: i32,
    subminor: i32,
) -> i32 {
    let version = pack_i32_os_version(major, minor, subminor);

    // Mac Catalyst 是一项允许 macOS 以一种与 iOS 高度相似的不同 "模式" 运行的技术
    //（并提供了像 UIKit 这样的 iOS 库）。
    //
    //（Apple 后来又加入了 "Designed for iPad" 模式，允许原生运行 iOS 应用，但我们
    // 无需对其想太多，因为它们同样链接到 iOS 专属的系统二进制）。
    //
    // 为了支持 Mac Catalyst，Apple 引入了 "zippered" 二进制的概念，即单个二进制既能
    // 在 macOS 上运行也能在 Mac Catalyst 上运行（带有两条 `LC_BUILD_VERSION` Mach-O
    // 命令，一条设为 `PLATFORM_MACOS`，一条设为 `PLATFORM_MACCATALYST`）。
    //
    // 大多数系统库都是 zippered 的，这使得它们可以在 macOS 与 Mac Catalyst 间复用。
    // 这其中就包括随 Xcode 一起分发的 `libclang_rt.osx.a`！这意味着 `compiler-rt`
    // 无法在静态层面知道自己是为 macOS 还是 Mac Catalyst 编译的，因此就需要这个新的
    // API（它替代了 `__isOSVersionAtLeast`）。
    //
    // 简而言之：
    //      普通二进制 调用 普通 compiler-rt --> `__isOSVersionAtLeast` 就足够了
    //      普通二进制 调用 zippered compiler-rt --> 需要 `__isPlatformVersionAtLeast`
    //     zippered 二进制 调用 zippered compiler-rt --> 调用 `__isPlatformOrVariantPlatformVersionAtLeast`

    // FIXME(madsmtm): `rustc` 尚不支持 zippered 二进制，参见 rust-lang/rust#131216。
    // 但一旦支持了，我们就需要让 rustup 分发的预编译 `std` 是 zippered 的，因而我们这里
    // 也需要处理 `platform` 的差异：
    //
    // if cfg!(target_os = "macos") && platform == 2 /* PLATFORM_IOS */ && cfg!(zippered) {
    //     return (version.to_u32() <= current_ios_version()) as i32;
    // }
    //
    // `__isPlatformOrVariantPlatformVersionAtLeast` 也将需要实现。

    // 当前目标的基础 Mach-O 平台。
    const BASE_TARGET_PLATFORM: i32 = if cfg!(target_os = "macos") {
        1 // PLATFORM_MACOS
    } else if cfg!(target_os = "ios") {
        2 // PLATFORM_IOS
    } else if cfg!(target_os = "tvos") {
        3 // PLATFORM_TVOS
    } else if cfg!(target_os = "watchos") {
        4 // PLATFORM_WATCHOS
    } else if cfg!(target_os = "visionos") {
        11 // PLATFORM_VISIONOS
    } else {
        0 // PLATFORM_UNKNOWN
    };
    debug_assert_eq!(
        platform, BASE_TARGET_PLATFORM,
        "invalid platform provided to __isPlatformVersionAtLeast",
    );

    (version <= current_version()) as i32
}

/// 旧的可用性检查入口点。在用较旧的 Clang 版本编译时使用。
// SAFETY: 与 `__isPlatformVersionAtLeast` 相同。
#[rustc_std_internal_symbol]
#[linkage = "weak"]
pub(super) extern "C" fn __isOSVersionAtLeast(major: i32, minor: i32, subminor: i32) -> i32 {
    let version = pack_i32_os_version(major, minor, subminor);
    (version <= current_version()) as i32
}
