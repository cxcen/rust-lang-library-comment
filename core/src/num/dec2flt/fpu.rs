//! 平台相关的汇编辅助，用来在带 FPU 的架构上避免中间舍入。

pub(super) use fpu_precision::set_precision;

// 在 x86 上，如果 SSE/SSE2 扩展不可用，浮点运算会使用 x87 FPU。x87 FPU 默认以 80 位
// 精度运算，这意味着中间结果先舍入到 80 位，最终落到 32/64 位浮点时可能再次舍入，
// 从而产生 double rounding。为避免这种情况，可以设置 FPU control word，让计算直接以
// 目标精度执行。
//
// 注意，通常在 Rust 代码运行期间修改 FPU control word 是 Undefined Behavior；编译器
// 假设该控制字始终保持默认状态。不过在这个特定场景里，修改后的控制字反而更贴近 Rust
// 期望的语义；可以说在 `set_precision` guard 作用域之外运行的代码才与 Rust 语义不合。
// 换言之，这里只是为了绕过 <https://github.com/rust-lang/rust/issues/114479>。
// 有时确实会用 UB 压住另一个 UB。
// 如果这里设置为 32 位精度，仍有风险：编译器可能把某些 64 位操作移动到
// `set_precision` guard 的作用域内。因此这并非完全 sound；但它并不比默认 80 位精度
// 状态更不 sound。
#[cfg(all(target_arch = "x86", not(target_feature = "sse2")))]
mod fpu_precision {
    use core::arch::asm;

    /// 保存 FPU control word 原始值的结构，便于在 drop 时恢复。
    ///
    /// x87 FPU control word 是 16 位寄存器，字段如下：
    ///
    /// | 12-15 | 10-11 | 8-9 | 6-7 |  5 |  4 |  3 |  2 |  1 |  0 |
    /// |------:|------:|----:|----:|---:|---:|---:|---:|---:|---:|
    /// |       | RC    | PC  |     | PM | UM | OM | ZM | DM | IM |
    ///
    /// 所有字段的文档见 IA-32 Architectures Software Developer's Manual（Volume 1）。
    ///
    /// 下方代码只关心 PC（Precision Control）字段。它决定 FPU 执行运算时使用的精度：
    ///  - 0b00，single precision，即 32 位
    ///  - 0b10，double precision，即 64 位
    ///  - 0b11，double extended precision，即 80 位（默认状态）
    /// 0b01 是保留值，不应使用。
    pub(crate) struct FPUControlWord(u16);

    fn set_cw(cw: u16) {
        // SAFETY: `fldcw` 指令已经过审查，能对任意 `u16` 控制字正确执行；这里只把
        // 栈上 `cw` 的地址传给汇编，不暴露 Rust 引用给外部代码。
        unsafe {
            asm!(
                "fldcw word ptr [{}]",
                in(reg) &cw,
                options(nostack),
            )
        }
    }

    /// 把 FPU 的 precision 字段设置为适合 `T` 的值，并返回保存原状态的 `FPUControlWord`。
    pub(crate) fn set_precision<T>() -> FPUControlWord {
        let mut cw = 0_u16;

        // 计算适合 `T` 的 Precision Control 字段值。
        let cw_precision = match size_of::<T>() {
            4 => 0x0000, // 32 位
            8 => 0x0200, // 64 位
            _ => 0x0300, // 默认值，80 位
        };

        // 取得 control word 原始值，稍后在 `FPUControlWord` drop 时恢复。
        // SAFETY: `fnstcw` 指令已经过审查，能对任意 `u16` 输出位置正确执行；这里传入的是
        // 有效的可写栈地址。
        unsafe {
            asm!(
                "fnstcw word ptr [{}]",
                in(reg) &mut cw,
                options(nostack),
            )
        }

        // 把 control word 设置为目标精度：先清掉旧 precision 位（第 8、9 位，0x300），
        // 再填入上面计算出的 precision 标志。
        set_cw((cw & 0xFCFF) | cw_precision);

        FPUControlWord(cw)
    }

    impl Drop for FPUControlWord {
        fn drop(&mut self) {
            set_cw(self.0)
        }
    }
}

// 在大多数架构上，浮点操作自带显式位宽，因此计算精度由每条操作本身决定。
#[cfg(any(not(target_arch = "x86"), target_feature = "sse2"))]
mod fpu_precision {
    pub(crate) fn set_precision<T>() {}
}
