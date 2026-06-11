//! configure builtins 为若干 compiler-builtin 特性提供运行时支持，这些特性
//! 需要动态初始化才能按预期工作，例如 aarch64 的
//! outline-atomics（外联原子操作）。

/// 在程序启动时启用 LSE 原子操作（若硬件支持）。
///
/// 这里所用的链接器段（linker section）参照 [`ctor`] 的做法，并设置了相应优先级，
/// 使其在用户代码之前略早运行：
///
/// - Apple 使用段 `__mod_init_func`，需要 `mod_init_funcs` 来设置
///   `S_MOD_INIT_FUNC_POINTERS`。这里似乎没有办法指定优先级。
/// - Windows 使用 `.CRT$XCT`，它在用户构造函数之前运行（用户构造函数应使用 `.CRT$XCU`）。
/// - ELF 使用 `.init_array`，优先级为 90，会在我们的 `ARGV_INIT_ARRAY`
///   初始化器（优先级 99）之前运行。两者都落在 0-100 这一实现保留区间内（见
///   [`prio-ctor-dtor`] 警告的文档），这与 compiler-rt 的 `CONSTRUCTOR_PRIORITY` 一致。
///
/// 为了节省启动时间，只有在可能用到 compiler-builtins 的外联原子例程时才运行该初始化器。
/// 如果已知 LSE 可用，则根本不会发出这些调用；而如果我们构建的是 C 版本的内建函数（intrinsics），
/// 那么它会通过符号 `__aarch64_have_lse_atomics` 使用自己的初始化器。
///
/// 初始化在一个全局构造函数中完成，这样无论是否使用 Rust 的 `init`，也无论我们处于
/// `dylib` 还是 `no_main` 场景（而不是作为 pre-main 启动的一部分来做），都能获得一致的行为。
/// 这也与 C 的实现方式相吻合。
///
/// 理想情况下 `core` 也应有类似机制，但检测 CPU 特性需要来自操作系统的辅助向量
/// （auxiliary vector）。我们之所以在 `std` 而非 `compiler-builtins` 中做初始化，是因为
/// builtins->std 的依赖不可行，而把 `std-detect` 的部分内容内联进来又会麻烦得多。
///
/// [`ctor`]: https://github.com/mmastrac/rust-ctor/blob/63382b833ddcbfb8b064f4e86bfa1ed4026ff356/shared/src/macros/mod.rs#L522-L534
/// [`prio-ctor-dtor`]: https://gcc.gnu.org/onlinedocs/gcc/Warning-Options.html
#[cfg(all(
    target_arch = "aarch64",
    target_feature = "outline-atomics",
    not(target_feature = "lse"),
    not(feature = "compiler-builtins-c"),
))]
#[used]
#[cfg_attr(target_vendor = "apple", unsafe(link_section = "__DATA,__mod_init_func,mod_init_funcs"))]
#[cfg_attr(target_os = "windows", unsafe(link_section = ".CRT$XCT"))]
#[cfg_attr(
    not(any(target_vendor = "apple", target_os = "windows")),
    unsafe(link_section = ".init_array.90")
)]
static RUST_LSE_INIT: extern "C" fn() = {
    extern "C" fn init_lse() {
        use crate::arch;

        // 这个函数由 compiler-builtins::aarch64_outline_atomics 提供。
        unsafe extern "C" {
            fn __rust_enable_lse();
        }

        if arch::is_aarch64_feature_detected!("lse") {
            unsafe {
                __rust_enable_lse();
            }
        }
    }
    init_lse
};
