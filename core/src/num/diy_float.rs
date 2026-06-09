//! 仅供内部使用的扩展精度“软浮点”表示。

// 该模块只服务于 dec2flt 和 flt2dec；公开到 crate 内部只是为了 coretests 能覆盖这些
// 舍入敏感路径。它不是稳定 API，也不应被数值转换子系统之外的代码依赖。
#![doc(hidden)]
#![unstable(
    feature = "core_private_diy_float",
    reason = "internal routines only exposed for testing",
    issue = "none"
)]

/// 自定义的 64 位二进制浮点中间表示，数值为 `f * 2^e`。
///
/// 它不表示 IEEE 754 的完整格式，而是 dec2flt/flt2dec 在十进制和二进制之间转换时
/// 使用的定点化近似。把尾数和指数显式拆开可以避免硬件浮点中间舍入影响最终结果，
/// 从而让算法自己控制“最接近、ties-to-even”等舍入规则。
#[derive(Copy, Clone, Debug)]
#[doc(hidden)]
pub struct Fp {
    /// 整数尾数。
    pub f: u64,
    /// 以 2 为底的指数。
    pub e: i16,
}

impl Fp {
    /// 返回 `self` 与 `other` 的正确舍入乘积。
    ///
    /// 乘法先得到 128 位乘积，再根据低半部分的最高位对高半部分做一次舍入。这是
    /// Grisu 等格式化算法常用的扩展精度乘法步骤，目的是在固定大小的 `u64` 尾数中
    /// 保留足够信息，避免十进制输出因中间截断而偏离正确舍入边界。
    pub fn mul(self, other: Self) -> Self {
        let (lo, hi) = self.f.widening_mul(other.f);
        let f = hi + (lo >> 63) /* round */;
        let e = self.e + other.e + 64;
        Self { f, e }
    }

    /// 规格化自身，使得到的尾数至少为 `2^63`。
    ///
    /// 规格化会把尾数左移到最高位附近，并同步降低二进制指数。这样后续比较、乘法和
    /// 边界计算都能假设尾数占满有效位宽，从而减少前导零导致的精度浪费。
    pub fn normalize(self) -> Self {
        let lz = self.f.leading_zeros();
        let f = self.f << lz;
        let e = self.e - lz as i16;
        debug_assert!(f >= (1 << 63));
        Self { f, e }
    }

    /// 把自身规格化到指定的共享指数。
    ///
    /// 该操作只能降低指数，因此会相应增大尾数；断言保证左移不会丢失信息。共享指数
    /// 让多个 `Fp` 值可以在同一二进制尺度下比较，这是十进制边界判定能保持精确的前提。
    pub fn normalize_to(self, e: i16) -> Self {
        let edelta = self.e - e;
        assert!(edelta >= 0);
        let edelta = edelta as usize;
        assert_eq!(self.f << edelta >> edelta, self.f);
        Self { f: self.f << edelta, e }
    }
}
