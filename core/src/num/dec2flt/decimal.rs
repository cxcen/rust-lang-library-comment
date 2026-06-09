//! 用十进制有效数字和指数表示一个待转换浮点值。
//!
//! 解析阶段把字符串归约为 `mantissa * 10^exponent` 加符号位。后续 fast path 和慢速路径
//! 都从这个结构出发，并根据 `many_digits` 判断是否可能存在被截断的有效数字。

use crate::num::dec2flt::float::RawFloat;
use crate::num::dec2flt::fpu::set_precision;

const INT_POW10: [u64; 16] = [
    1,
    10,
    100,
    1000,
    10000,
    100000,
    1000000,
    10000000,
    100000000,
    1000000000,
    10000000000,
    100000000000,
    1000000000000,
    10000000000000,
    100000000000000,
    1000000000000000,
];

/// mantissa 最多 64 位、exponent 为 `i64` 的十进制浮点中间表示。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Decimal {
    pub exponent: i64,
    pub mantissa: u64,
    pub negative: bool,
    pub many_digits: bool,
}

impl Decimal {
    /// 判断该十进制值是否能用原生浮点 fast path 精确重建。
    #[inline]
    fn can_use_fast_path<F: RawFloat>(&self) -> bool {
        F::MIN_EXPONENT_FAST_PATH <= self.exponent
            && self.exponent <= F::MAX_EXPONENT_DISGUISED_FAST_PATH
            && self.mantissa <= F::MAX_MANTISSA_FAST_PATH
            && !self.many_digits
    }

    /// 尝试使用机器字大小整数和浮点数，把该十进制值转换为精确浮点表示。
    ///
    /// 只有 mantissa 和 exponent 都能在机器浮点中精确表示时，才能走该路径；此时 IEEE 754
    /// 保证中间运算不会发生额外舍入，从而最终结果就是正确舍入结果。
    ///
    /// 例外是 disguised fast-path：可以把一部分 10 的幂从 exponent 转移到有效数字中，
    /// 让剩余指数落入 fast path 支持范围。
    pub fn try_fast_path<F: RawFloat>(&self) -> Option<F> {
        // 这里需要绕过 <https://github.com/rust-lang/rust/issues/114479>。
        // fast path 依赖算术直接舍入到正确位数，不能出现中间舍入。在没有 SSE/SSE2 的 x86
        // 上，这要求修改 x87 FPU 栈精度，使其直接舍入到 64/32 位。`set_precision`
        // 会在需要通过全局状态（例如 x87 FPU 控制字）设置精度的架构上完成这项工作。
        let _cw = set_precision::<F>();

        if !self.can_use_fast_path::<F>() {
            return None;
        }

        let value = if self.exponent <= F::MAX_EXPONENT_FAST_PATH {
            // 普通 fast path。
            let value = F::from_u64(self.mantissa);
            if self.exponent < 0 {
                value / F::pow10_fast_path((-self.exponent) as _)
            } else {
                value * F::pow10_fast_path(self.exponent as _)
            }
        } else {
            // 伪装的快速路径。
            let shift = self.exponent - F::MAX_EXPONENT_FAST_PATH;
            let mantissa = self.mantissa.checked_mul(INT_POW10[shift as usize])?;
            if mantissa > F::MAX_MANTISSA_FAST_PATH {
                return None;
            }
            F::from_u64(mantissa) * F::pow10_fast_path(F::MAX_EXPONENT_FAST_PATH as _)
        };

        if self.negative { Some(-value) } else { Some(value) }
    }
}
