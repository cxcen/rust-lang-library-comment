//! 部分 intrinsic 的“回退实现”（fallback）。
//!
//! 设计背景：intrinsic 是编译器内建操作，是 core 与编译器（rustc/LLVM）之间的契约层。
//! 对某些 intrinsic，编译器会用一段“回退 MIR”来实现：当目标后端没有对应的原生支持时，
//! 就退化为调用这里用纯 Rust 写出的等价实现。这些 fallback 永远不会稳定（见下方 feature 说明），
//! 它们存在的唯一目的就是被回退 MIR 调用；之所以 `pub` 导出，是为了能在那些实际不使用回退 MIR
//! 的平台上对它们做测试。

#![unstable(
    feature = "core_intrinsics_fallbacks",
    reason = "The fallbacks will never be stable, as they exist only to be called \
              by the fallback MIR, but they're exported so they can be tested on \
              platforms where the fallback MIR isn't actually used",
    issue = "none"
)]
#![allow(missing_docs)]

#[rustc_const_unstable(feature = "core_intrinsics_fallbacks", issue = "none")]
pub const trait CarryingMulAdd: Copy + 'static {
    type Unsigned: Copy + 'static;
    fn carrying_mul_add(
        self,
        multiplicand: Self,
        addend: Self,
        carry: Self,
    ) -> (Self::Unsigned, Self);
}

macro_rules! impl_carrying_mul_add_by_widening {
    ($($t:ident $u:ident $w:ident,)+) => {$(
        #[rustc_const_unstable(feature = "core_intrinsics_fallbacks", issue = "none")]
        impl const CarryingMulAdd for $t {
            type Unsigned = $u;
            #[inline]
            fn carrying_mul_add(self, a: Self, b: Self, c: Self) -> ($u, $t) {
                let wide = (self as $w) * (a as $w) + (b as $w) + (c as $w);
                (wide as _, (wide >> Self::BITS) as _)
            }
        }
    )+};
}
impl_carrying_mul_add_by_widening! {
    u8 u8 u16,
    u16 u16 u32,
    u32 u32 u64,
    u64 u64 u128,
    usize usize UDoubleSize,
    i8 u8 i16,
    i16 u16 i32,
    i32 u32 i64,
    i64 u64 i128,
    isize usize UDoubleSize,
}

#[cfg(target_pointer_width = "16")]
type UDoubleSize = u32;
#[cfg(target_pointer_width = "32")]
type UDoubleSize = u64;
#[cfg(target_pointer_width = "64")]
type UDoubleSize = u128;

#[inline]
const fn wide_mul_u128(a: u128, b: u128) -> (u128, u128) {
    #[inline]
    const fn to_low_high(x: u128) -> [u128; 2] {
        const MASK: u128 = u64::MAX as _;
        [x & MASK, x >> 64]
    }
    #[inline]
    const fn from_low_high(x: [u128; 2]) -> u128 {
        x[0] | (x[1] << 64)
    }
    #[inline]
    const fn scalar_mul(low_high: [u128; 2], k: u128) -> [u128; 3] {
        let [x, c] = to_low_high(k * low_high[0]);
        let [y, z] = to_low_high(k * low_high[1] + c);
        [x, y, z]
    }
    let a = to_low_high(a);
    let b = to_low_high(b);
    let low = scalar_mul(a, b[0]);
    let high = scalar_mul(a, b[1]);
    let r0 = low[0];
    let [r1, c] = to_low_high(low[1] + high[0]);
    let [r2, c] = to_low_high(low[2] + high[1] + c);
    let r3 = high[2] + c;
    (from_low_high([r0, r1]), from_low_high([r2, r3]))
}

#[rustc_const_unstable(feature = "core_intrinsics_fallbacks", issue = "none")]
impl const CarryingMulAdd for u128 {
    type Unsigned = u128;
    #[inline]
    fn carrying_mul_add(self, b: u128, c: u128, d: u128) -> (u128, u128) {
        let (low, mut high) = wide_mul_u128(self, b);
        let (low, carry) = u128::overflowing_add(low, c);
        high += carry as u128;
        let (low, carry) = u128::overflowing_add(low, d);
        high += carry as u128;
        (low, high)
    }
}

#[rustc_const_unstable(feature = "core_intrinsics_fallbacks", issue = "none")]
impl const CarryingMulAdd for i128 {
    type Unsigned = u128;
    #[inline]
    fn carrying_mul_add(self, b: i128, c: i128, d: i128) -> (u128, i128) {
        let (low, high) = wide_mul_u128(self as u128, b as u128);
        let mut high = high as i128;
        high = high.wrapping_add(i128::wrapping_mul(self >> 127, b));
        high = high.wrapping_add(i128::wrapping_mul(self, b >> 127));
        let (low, carry) = u128::overflowing_add(low, c as u128);
        high = high.wrapping_add((carry as i128) + (c >> 127));
        let (low, carry) = u128::overflowing_add(low, d as u128);
        high = high.wrapping_add((carry as i128) + (d >> 127));
        (low, high)
    }
}

#[rustc_const_unstable(feature = "core_intrinsics_fallbacks", issue = "none")]
pub const trait DisjointBitOr: Copy + 'static {
    /// 参见 [`super::disjoint_bitor`]；这里需要借助 trait 做一层间接，
    /// 因为带泛型地直接调用 intrinsic 行不通，只能为不同具体类型分别实现。
    unsafe fn disjoint_bitor(self, other: Self) -> Self;
}
macro_rules! zero {
    (bool) => {
        false
    };
    ($t:ident) => {
        0
    };
}
macro_rules! impl_disjoint_bitor {
    ($($t:ident,)+) => {$(
        #[rustc_const_unstable(feature = "core_intrinsics_fallbacks", issue = "none")]
        impl const DisjointBitOr for $t {
            #[cfg_attr(miri, track_caller)]
            #[inline]
            unsafe fn disjoint_bitor(self, other: Self) -> Self {
                // 注意：这里的 assume 是 Miri 进行 UB 检测所必需的！

                // SAFETY: 前置条件要求两个操作数没有任何公共置位的 bit，
                // 所以这里只是把这个事实告知后端（让其据此做位或的优化）。
                unsafe { super::assume((self & other) == zero!($t)) };
                self | other
            }
        }
    )+};
}
impl_disjoint_bitor! {
    bool,
    u8, u16, u32, u64, u128, usize,
    i8, i16, i32, i64, i128, isize,
}

#[rustc_const_unstable(feature = "core_intrinsics_fallbacks", issue = "none")]
pub const trait FunnelShift: Copy + 'static {
    /// 参见 [`super::unchecked_funnel_shl`]；这里需要借助 trait 做一层间接，
    /// 因为带泛型地直接调用 intrinsic 行不通，只能为不同具体类型分别实现。
    unsafe fn unchecked_funnel_shl(self, rhs: Self, shift: u32) -> Self;

    /// 参见 [`super::unchecked_funnel_shr`]；这里需要借助 trait 做一层间接，
    /// 因为带泛型地直接调用 intrinsic 行不通，只能为不同具体类型分别实现。
    unsafe fn unchecked_funnel_shr(self, rhs: Self, shift: u32) -> Self;
}

macro_rules! impl_funnel_shifts {
    ($($type:ident),*) => {$(
        #[rustc_const_unstable(feature = "core_intrinsics_fallbacks", issue = "none")]
        impl const FunnelShift for $type {
            #[cfg_attr(miri, track_caller)]
            #[inline]
            unsafe fn unchecked_funnel_shl(self, rhs: Self, shift: u32) -> Self {
                // 该实现同样被 Miri 使用，所以这里必须检查前置条件。
                // SAFETY: 这一点由调用方保证（即 shift < 该类型位宽）。
                unsafe { super::assume(shift < $type::BITS) };
                if shift == 0 {
                    self
                } else {
                    // SAFETY:
                    //  - `shift < T::BITS`，满足 `unchecked_shl` 的前置；
                    //  - 这同时保证了 `T::BITS - shift < T::BITS`（shift == 0 已在上面排除），
                    //    满足 `unchecked_shr` 的前置；
                    //  - 因为是无符号类型，两部分拼接时所占的 bit 互不重叠（即 disjoint）。
                    //    若是有符号类型则不成立，因为 SHR 会用符号位而非 0 来填补空出的高位。
                    unsafe {
                        super::disjoint_bitor(
                            super::unchecked_shl(self, shift),
                            super::unchecked_shr(rhs, $type::BITS - shift),
                        )
                    }
                }
            }

            #[cfg_attr(miri, track_caller)]
            #[inline]
            unsafe fn unchecked_funnel_shr(self, rhs: Self, shift: u32) -> Self {
                // 该实现同样被 Miri 使用，所以这里必须检查前置条件。
                // SAFETY: 这一点由调用方保证（即 shift < 该类型位宽）。
                unsafe { super::assume(shift < $type::BITS) };
                if shift == 0 {
                    rhs
                } else {
                    // SAFETY:
                    //  - `shift < T::BITS`，满足 `unchecked_shr` 的前置；
                    //  - 这同时保证了 `T::BITS - shift < T::BITS`（shift == 0 已在上面排除），
                    //    满足 `unchecked_shl` 的前置；
                    //  - 因为是无符号类型，两部分拼接时所占的 bit 互不重叠（即 disjoint）。
                    //    若是有符号类型则不成立，因为 SHR 会用符号位而非 0 来填补空出的高位。
                    unsafe {
                        super::disjoint_bitor(
                            super::unchecked_shl(self, $type::BITS - shift),
                            super::unchecked_shr(rhs, shift),
                        )
                    }
                }
            }
        }
    )*};
}

impl_funnel_shifts! {
    u8, u16, u32, u64, u128, usize
}
