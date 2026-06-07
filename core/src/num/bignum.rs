//! 自定义任意精度整数(bignum)实现。
//!
//! 该实现用固定大小数组换取无堆分配：常用的 `Big32x40` 上限是 32 × 40 = 1,280 位，
//! 最多占用 160 字节栈空间。这个容量足以覆盖所有有限 `f64` 值在 dec2flt/flt2dec
//! 往返转换时需要的十进制/二进制中间精度。
//!
//! 理论上可以为不同输入准备多种 bignum 类型，但那会显著增加代码体积。这里复用少数
//! 固定容量类型，并在每个值中记录实际使用的 digit 数量；在正常路径上只处理有效前缀，
//! 因此不会为了最坏情况付出过多运行时成本。

// 该模块只服务于 dec2flt 和 flt2dec；公开到 crate 内部只是为了 coretests 能覆盖这些
// 舍入敏感路径。它不是稳定 API，也不应被数值转换子系统之外的代码依赖。
#![doc(hidden)]
#![unstable(
    feature = "core_private_bignum",
    reason = "internal routines only exposed for testing",
    issue = "none"
)]
#![macro_use]

/// bignum digit 运算需要的全宽算术操作。
///
/// 单个 digit 的乘加或除法会产生比 digit 本身更宽的中间结果。该 trait 把“低位结果”和
/// “进位/余数”显式返回，避免 bignum 算法依赖隐式溢出语义。
pub trait FullOps: Sized {
    /// 返回 `(carry', v')`，满足
    /// `carry' * 2^W + v' = self * other + other2 + carry`。
    ///
    /// 其中 `W` 是 `Self` 的位宽。`v'` 是低 `W` 位，`carry'` 是剩余高位。
    fn full_mul_add(self, other: Self, other2: Self, carry: Self) -> (Self /* carry */, Self);

    /// 返回 `(quo, rem)`，满足 `borrow * 2^W + self = quo * other + rem`，
    /// 且 `0 <= rem < other`。
    ///
    /// 其中 `W` 是 `Self` 的位宽。调用方必须保证 `borrow < other`，这样合成的被除数
    /// 可以用更宽整数精确表示，商也能落回一个 digit。
    fn full_div_rem(self, other: Self, borrow: Self)
    -> (Self /* quotient */, Self /* remainder */);
}

macro_rules! impl_full_ops {
    ($($ty:ty: add($addfn:path), mul/div($bigty:ident);)*) => (
        $(
            impl FullOps for $ty {
                fn full_mul_add(self, other: $ty, other2: $ty, carry: $ty) -> ($ty, $ty) {
                    // 这里不会溢出：输出范围在 `0` 到 `2^nbits * (2^nbits - 1)` 之间，
                    // `carrying_mul_add` 会把高低位分开返回。
                    let (lo, hi) = self.carrying_mul_add(other, other2, carry);
                    (hi, lo)
                }

                fn full_div_rem(self, other: $ty, borrow: $ty) -> ($ty, $ty) {
                    debug_assert!(borrow < other);
                    // 这里不会溢出：`borrow < other`，所以合成被除数小于
                    // `other * 2^nbits`，商最多为 `2^nbits - 1`。
                    let lhs = ((borrow as $bigty) << <$ty>::BITS) | (self as $bigty);
                    let rhs = other as $bigty;
                    ((lhs / rhs) as $ty, (lhs % rhs) as $ty)
                }
            }
        )*
    )
}

impl_full_ops! {
    u8:  add(intrinsics::u8_add_with_overflow),  mul/div(u16);
    u16: add(intrinsics::u16_add_with_overflow), mul/div(u32);
    u32: add(intrinsics::u32_add_with_overflow), mul/div(u64);
    u64: add(intrinsics::u64_add_with_overflow), mul/div(u128);
}

/// 可以放入单个 digit 的 5 的幂表。
///
/// 每项记录 `{u8, u16, u32}` 对应 digit 中最大的 5 的幂，以及该幂的指数。`mul_pow5`
/// 先尽量乘这些单 digit 幂，再处理余下指数，从而减少多精度乘法次数。
const SMALL_POW5: [(u64, usize); 3] = [(125, 3), (15625, 6), (1_220_703_125, 13)];

macro_rules! define_bignum {
    ($name:ident: type=$ty:ty, n=$n:expr) => {
        /// 栈上分配的有限容量任意精度整数。
        ///
        /// 该类型由固定大小的 digit 数组支撑。数组通常只有数百字节，但在格式化热路径中
        /// 随意复制仍会带来可见成本，因此它刻意不实现 `Copy`。
        ///
        /// bignum 的所有操作在容量不足时都会 panic。调用方负责选择足够大的类型；
        /// 这条前置是 dec2flt/flt2dec 正确舍入保证的一部分，因为中间值被截断会直接改变
        /// 舍入边界判断。
        pub struct $name {
            /// 当前使用的最高 digit 下标加一。
            ///
            /// 该值不会主动缩小，因此调用方需要注意计算顺序。`base[size..]` 应保持为零，
            /// 这样比较和 Debug 输出可以只关注有效前缀。
            size: usize,
            /// digit 数组。`[a, b, c, ...]` 表示
            /// `a + b*2^W + c*2^(2W) + ...`，其中 `W` 是 digit 类型的位宽。
            base: [$ty; $n],
        }

        impl $name {
            /// 从单个 digit 构造 bignum。
            pub fn from_small(v: $ty) -> $name {
                let mut base = [0; $n];
                base[0] = v;
                $name { size: 1, base }
            }

            /// 从 `u64` 值构造 bignum。
            pub fn from_u64(mut v: u64) -> $name {
                let mut base = [0; $n];
                let mut sz = 0;
                while v > 0 {
                    base[sz] = v as $ty;
                    v >>= <$ty>::BITS;
                    sz += 1;
                }
                $name { size: sz, base }
            }

            /// 以切片形式返回内部 digit。
            ///
            /// 切片 `[a, b, c, ...]` 的数值是 `a + b * 2^W + c * 2^(2W) + ...`，
            /// 其中 `W` 是 digit 类型的位宽。
            pub fn digits(&self) -> &[$ty] {
                &self.base[..self.size]
            }

            /// 返回第 `i` 位，bit 0 是最低有效位。
            ///
            /// 换言之，返回权重为 `2^i` 的那一位。
            pub fn get_bit(&self, i: usize) -> u8 {
                let digitbits = <$ty>::BITS as usize;
                let d = i / digitbits;
                let b = i % digitbits;
                ((self.base[d] >> b) & 1) as u8
            }

            /// 如果 bignum 为零则返回 `true`。
            pub fn is_zero(&self) -> bool {
                self.digits().iter().all(|&v| v == 0)
            }

            /// 返回表示该值所需的位数。
            ///
            /// 零被视为需要 0 位。
            pub fn bit_length(&self) -> usize {
                let digitbits = <$ty>::BITS as usize;
                let digits = self.digits();
                // 找到最高的非零 digit。
                let msd = digits.iter().rposition(|&x| x != 0);
                match msd {
                    Some(msd) => msd * digitbits + digits[msd].ilog2() as usize + 1,
                    // 不存在非零 digit，说明该数为零。
                    _ => 0,
                }
            }

            /// 把 `other` 加到自身，并返回自身的可变引用。
            pub fn add<'a>(&'a mut self, other: &$name) -> &'a mut $name {
                use crate::{cmp, iter};

                let mut sz = cmp::max(self.size, other.size);
                let mut carry = false;
                for (a, b) in iter::zip(&mut self.base[..sz], &other.base[..sz]) {
                    let (v, c) = (*a).carrying_add(*b, carry);
                    *a = v;
                    carry = c;
                }
                if carry {
                    self.base[sz] = 1;
                    sz += 1;
                }
                self.size = sz;
                self
            }

            pub fn add_small(&mut self, other: $ty) -> &mut $name {
                let (v, mut carry) = self.base[0].carrying_add(other, false);
                self.base[0] = v;
                let mut i = 1;
                while carry {
                    let (v, c) = self.base[i].carrying_add(0, carry);
                    self.base[i] = v;
                    carry = c;
                    i += 1;
                }
                if i > self.size {
                    self.size = i;
                }
                self
            }

            /// 从自身减去 `other`，并返回自身的可变引用。
            pub fn sub<'a>(&'a mut self, other: &$name) -> &'a mut $name {
                use crate::{cmp, iter};

                let sz = cmp::max(self.size, other.size);
                let mut noborrow = true;
                for (a, b) in iter::zip(&mut self.base[..sz], &other.base[..sz]) {
                    let (v, c) = (*a).carrying_add(!*b, noborrow);
                    *a = v;
                    noborrow = c;
                }
                assert!(noborrow);
                self.size = sz;
                self
            }

            /// 把自身乘以一个 digit 大小的 `other`，并返回自身的可变引用。
            pub fn mul_small(&mut self, other: $ty) -> &mut $name {
                let mut sz = self.size;
                let mut carry = 0;
                for a in &mut self.base[..sz] {
                    let (v, c) = (*a).carrying_mul(other, carry);
                    *a = v;
                    carry = c;
                }
                if carry > 0 {
                    self.base[sz] = carry;
                    sz += 1;
                }
                self.size = sz;
                self
            }

            /// 把自身乘以 `2^bits`，并返回自身的可变引用。
            pub fn mul_pow2(&mut self, bits: usize) -> &mut $name {
                let digitbits = <$ty>::BITS as usize;
                let digits = bits / digitbits;
                let bits = bits % digitbits;

                assert!(digits < $n);
                debug_assert!(self.base[$n - digits..].iter().all(|&v| v == 0));
                debug_assert!(bits == 0 || (self.base[$n - digits - 1] >> (digitbits - bits)) == 0);

                // 先按整 digit 数移动，也就是移动 `digits * digitbits` 位。
                for i in (0..self.size).rev() {
                    self.base[i + digits] = self.base[i];
                }
                for i in 0..digits {
                    self.base[i] = 0;
                }

                // 再移动剩余的 `bits` 位。
                let mut sz = self.size + digits;
                if bits > 0 {
                    let last = sz;
                    let overflow = self.base[last - 1] >> (digitbits - bits);
                    if overflow > 0 {
                        self.base[last] = overflow;
                        sz += 1;
                    }
                    for i in (digits + 1..last).rev() {
                        self.base[i] =
                            (self.base[i] << bits) | (self.base[i - 1] >> (digitbits - bits));
                    }
                    self.base[digits] <<= bits;
                    // `self.base[..digits]` 已经是零，不需要再移动。
                }

                self.size = sz;
                self
            }

            /// 把自身乘以 `5^e`，并返回自身的可变引用。
            pub fn mul_pow5(&mut self, mut e: usize) -> &mut $name {
                use crate::num::bignum::SMALL_POW5;

                // `2^n` 恰好有 n 个尾随零，而这里相关的 digit 大小都是连续的 2 的幂，
                // 因此可以用 digit 字节数的尾随零个数作为表索引。
                let table_index = size_of::<$ty>().trailing_zeros() as usize;
                let (small_power, small_e) = SMALL_POW5[table_index];
                let small_power = small_power as $ty;

                // 先尽可能多地乘以最大的单 digit 幂。
                while e >= small_e {
                    self.mul_small(small_power);
                    e -= small_e;
                }

                // 再处理剩余指数。
                let mut rest_power = 1;
                for _ in 0..e {
                    rest_power *= 5;
                }
                self.mul_small(rest_power);

                self
            }

            /// 把自身乘以由 `other[0] + other[1] * 2^W + other[2] * 2^(2W) + ...`
            /// 描述的数，并返回自身的可变引用。
            ///
            /// 其中 `W` 是 digit 类型的位宽。
            pub fn mul_digits<'a>(&'a mut self, other: &[$ty]) -> &'a mut $name {
                // 内部乘法例程；当 `aa.len() <= bb.len()` 时效率最好。
                fn mul_inner(ret: &mut [$ty; $n], aa: &[$ty], bb: &[$ty]) -> usize {
                    use crate::num::bignum::FullOps;

                    let mut retsz = 0;
                    for (i, &a) in aa.iter().enumerate() {
                        if a == 0 {
                            continue;
                        }
                        let mut sz = bb.len();
                        let mut carry = 0;
                        for (j, &b) in bb.iter().enumerate() {
                            let (c, v) = a.full_mul_add(b, ret[i + j], carry);
                            ret[i + j] = v;
                            carry = c;
                        }
                        if carry > 0 {
                            ret[i + sz] = carry;
                            sz += 1;
                        }
                        if retsz < i + sz {
                            retsz = i + sz;
                        }
                    }
                    retsz
                }

                let mut ret = [0; $n];
                let retsz = if self.size < other.len() {
                    mul_inner(&mut ret, &self.digits(), other)
                } else {
                    mul_inner(&mut ret, other, &self.digits())
                };
                self.base = ret;
                self.size = retsz;
                self
            }

            /// 把自身除以一个 digit 大小的 `other`，并返回自身的可变引用和余数。
            pub fn div_rem_small(&mut self, other: $ty) -> (&mut $name, $ty) {
                use crate::num::bignum::FullOps;

                assert!(other > 0);

                let sz = self.size;
                let mut borrow = 0;
                for a in self.base[..sz].iter_mut().rev() {
                    let (q, r) = (*a).full_div_rem(other, borrow);
                    *a = q;
                    borrow = r;
                }
                (self, borrow)
            }
        }

        impl crate::cmp::PartialEq for $name {
            fn eq(&self, other: &$name) -> bool {
                self.base[..] == other.base[..]
            }
        }

        impl crate::cmp::Eq for $name {}

        impl crate::cmp::PartialOrd for $name {
            fn partial_cmp(&self, other: &$name) -> crate::option::Option<crate::cmp::Ordering> {
                crate::option::Option::Some(self.cmp(other))
            }
        }

        impl crate::cmp::Ord for $name {
            fn cmp(&self, other: &$name) -> crate::cmp::Ordering {
                use crate::cmp::max;
                let sz = max(self.size, other.size);
                let lhs = self.base[..sz].iter().cloned().rev();
                let rhs = other.base[..sz].iter().cloned().rev();
                lhs.cmp(rhs)
            }
        }

        impl crate::clone::Clone for $name {
            fn clone(&self) -> Self {
                Self { size: self.size, base: self.base }
            }
        }

        impl crate::clone::UseCloned for $name {}

        impl crate::fmt::Debug for $name {
            fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
                let sz = if self.size < 1 { 1 } else { self.size };
                let digitlen = <$ty>::BITS as usize / 4;

                write!(f, "{:#x}", self.base[sz - 1])?;
                for &v in self.base[..sz - 1].iter().rev() {
                    write!(f, "_{:01$x}", v, digitlen)?;
                }
                crate::result::Result::Ok(())
            }
        }
    };
}

/// `Big32x40` 使用的 digit 类型。
pub type Digit32 = u32;

define_bignum!(Big32x40: type=Digit32, n=40);

// 这个小容量 bignum 只用于测试。
#[doc(hidden)]
pub mod tests {
    define_bignum!(Big8x3: type=u8, n=3);
}
