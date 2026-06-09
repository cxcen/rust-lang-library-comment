//! 在调用方已经确认输入严格为正的前提下，计算各整数类型的十进制整数对数。
//!
//! 公共 API 需要在非正输入时 panic 或返回 `None`，而这里的内部函数只处理热路径。
//! 对无符号类型，`NonZero` 参数把“非零”前置条件编码进类型；对有符号类型，包装函数
//! 先过滤 `<= 0`。算法返回 `floor(log10(n))`，也就是十进制表示所需位数减一。

use crate::num::NonZero;

// 0 < val <= u8::MAX。
#[inline]
const fn u8_impl(val: u8) -> u32 {
    let val = val as u32;

    // 为了提升性能，避免分支判断；通过在低 8 位之上的高位拼出答案。

    // `val < 10` 时加上 c1 会让高位为 10，`val >= 10` 时高位为 11。
    const C1: u32 = 0b11_00000000 - 10; // 758
    // `val < 100` 时加上 c2 会让高位为 01，`val >= 100` 时高位为 10。
    const C2: u32 = 0b10_00000000 - 100; // 412

    // 高位组合出的结果如下：
    //            +c1  +c2  1&2
    //     0..=9   10   01   00 = 0
    //   10..=99   11   01   01 = 1
    // 100..=255   11   10   10 = 2
    ((val + C1) & (val + C2)) >> 8
}

// 0 < val < 100_000。
#[inline]
const fn less_than_5(val: u32) -> u32 {
    // 与 u8 路径类似，把常量加到 val 后，会根据 val 是否低于阈值，在低 17 位之上
    // 得到两种可能的位模式。
    const C1: u32 = 0b011_00000000000000000 - 10; // 393206
    const C2: u32 = 0b100_00000000000000000 - 100; // 524188
    const C3: u32 = 0b111_00000000000000000 - 1000; // 916504
    const C4: u32 = 0b100_00000000000000000 - 10000; // 514288

    // 高位组合出的结果如下：
    //                +c1  +c2  1&2  +c3  +c4  3&4   ^
    //         0..=9  010  011  010  110  011  010  000 = 0
    //       10..=99  011  011  011  110  011  010  001 = 1
    //     100..=999  011  100  000  110  011  010  010 = 2
    //   1000..=9999  011  100  000  111  011  011  011 = 3
    // 10000..=99999  011  100  000  111  100  100  100 = 4
    (((val + C1) & (val + C2)) ^ ((val + C3) & (val + C4))) >> 17
}

// 0 < val <= u16::MAX。
#[inline]
const fn u16_impl(val: u16) -> u32 {
    less_than_5(val as u32)
}

// 0 < val <= u32::MAX。
#[inline]
const fn u32_impl(mut val: u32) -> u32 {
    let mut log = 0;
    if val >= 100_000 {
        val /= 100_000;
        log += 5;
    }
    log + less_than_5(val)
}

// 0 < val <= u64::MAX。
#[inline]
const fn u64_impl(mut val: u64) -> u32 {
    let mut log = 0;
    if val >= 10_000_000_000 {
        val /= 10_000_000_000;
        log += 10;
    }
    if val >= 100_000 {
        val /= 100_000;
        log += 5;
    }
    log + less_than_5(val as u32)
}

// 0 < val <= u128::MAX。
#[inline]
const fn u128_impl(mut val: u128) -> u32 {
    let mut log = 0;
    if val >= 100_000_000_000_000_000_000_000_000_000_000 {
        val /= 100_000_000_000_000_000_000_000_000_000_000;
        log += 32;
        return log + u32_impl(val as u32);
    }
    if val >= 10_000_000_000_000_000 {
        val /= 10_000_000_000_000_000;
        log += 16;
    }
    log + u64_impl(val as u64)
}

macro_rules! define_unsigned_ilog10 {
    ($($ty:ident => $impl_fn:ident,)*) => {$(
        #[inline]
        pub(super) const fn $ty(val: NonZero<$ty>) -> u32 {
            let result = $impl_fn(val.get());

            // SAFETY: 整数对数单调不减，因此计算出的 `result` 不可能超过最大输入对应的值。
            unsafe { crate::hint::assert_unchecked(result <= const { $impl_fn($ty::MAX) }) };

            result
        }
    )*};
}

define_unsigned_ilog10! {
    u8 => u8_impl,
    u16 => u16_impl,
    u32 => u32_impl,
    u64 => u64_impl,
    u128 => u128_impl,
}

#[inline]
pub(super) const fn usize(val: NonZero<usize>) -> u32 {
    #[cfg(target_pointer_width = "16")]
    let impl_fn = u16;

    #[cfg(target_pointer_width = "32")]
    let impl_fn = u32;

    #[cfg(target_pointer_width = "64")]
    let impl_fn = u64;

    // SAFETY: 已根据目标指针宽度选择正确的 `impl_fn`，把 `usize` 的非零值转换成对应参数
    // 类型不会截断；原始 `NonZero` 的非零不变量也保持成立。
    impl_fn(unsafe { NonZero::new_unchecked(val.get() as _) })
}

macro_rules! define_signed_ilog10 {
    ($($ty:ident => $impl_fn:ident,)*) => {$(
        // 0 < val <= $ty::MAX。
        #[inline]
        pub(super) const fn $ty(val: $ty) -> Option<u32> {
            if val > 0 {
                let result = $impl_fn(val.cast_unsigned());

                // SAFETY: 整数对数单调不减，因此计算出的 `result` 不会超过最大输入对应的值。
                unsafe {
                    crate::hint::assert_unchecked(result <= const { $impl_fn($ty::MAX.cast_unsigned()) });
                }

                Some(result)
            } else {
                None
            }
        }
    )*};
}

define_signed_ilog10! {
    i8 => u8_impl,
    i16 => u16_impl,
    i32 => u32_impl,
    i64 => u64_impl,
    i128 => u128_impl,
}

/// 只实例化一次 `ilog` 的 panic 逻辑，避免每个原始整数类型上的每个 `ilog` 方法都生成一份。
#[cold]
#[track_caller]
pub(super) const fn panic_for_nonpositive_argument() -> ! {
    panic!("argument of integer logarithm must be positive")
}
