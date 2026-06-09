//! 使用 [Karatsuba square root algorithm][1] 为原始整数类型计算
//! [integer square root](https://en.wikipedia.org/wiki/Integer_square_root)。
//!
//! 对无符号整数，结果是满足 `s * s <= n < (s + 1) * (s + 1)` 的最大整数 `s`。
//! 对有符号整数，这里的内部函数只接受**非负**输入；公共方法必须在调用前处理负数，
//! 因为负数没有普通整数平方根。算法对大整数先做规格化，分阶段求商和余数，避免直接
//! 计算平方时溢出。
//!
//! [1]: <https://web.archive.org/web/20230511212802/https://inria.hal.science/inria-00072854v1/file/RR-3805.pdf>
//! "Paul Zimmermann. Karatsuba Square Root. \[Research Report\] RR-3805,
//! INRIA. 1999, pp.8. (inria-00072854)"

/// 保存每个 [`u8`](prim@u8) 值的
/// [integer square root](https://en.wikipedia.org/wiki/Integer_square_root)
/// 和余数。
///
/// 例如 `U8_ISQRT_WITH_REMAINDER[17]` 为 `(4, 1)`，因为 17 的整数平方根是 4，
/// 且 17 比 `4 * 4` 大 1。小表能让后续 Karatsuba 阶段从一个精确基例开始。
const U8_ISQRT_WITH_REMAINDER: [(u8, u8); 256] = {
    let mut result = [(0, 0); 256];

    let mut n: usize = 0;
    let mut isqrt_n: usize = 0;
    while n < result.len() {
        result[n] = (isqrt_n as u8, (n - isqrt_n.pow(2)) as u8);

        n += 1;
        if n == (isqrt_n + 1).pow(2) {
            isqrt_n += 1;
        }
    }

    result
};

/// 返回任意 [`u8`](prim@u8) 输入的
/// [integer square root](https://en.wikipedia.org/wiki/Integer_square_root)。
#[must_use = "this returns the result of the operation, \
              without modifying the original"]
#[inline]
pub(super) const fn u8(n: u8) -> u8 {
    U8_ISQRT_WITH_REMAINDER[n as usize].0
}

/// 生成一个 `i*` 函数，用来返回某个有符号整数类型的**非负**输入的
/// [integer square root](https://en.wikipedia.org/wiki/Integer_square_root)。
macro_rules! signed_fn {
    ($SignedT:ident, $UnsignedT:ident) => {
        /// 返回任意**非负**
        #[doc = concat!("[`", stringify!($SignedT), "`](prim@", stringify!($SignedT), ")")]
        /// input.
        ///
        /// # 安全性(Safety）
        ///
        /// `n` 必须大于等于 0。若传入负数，后续把它当作无符号数处理会破坏算法前置，
        /// 并可能让 `assert_unchecked` 的优化承诺变为假；因此负数输入是 undefined behavior。
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub(super) const unsafe fn $SignedT(n: $SignedT) -> $SignedT {
            debug_assert!(n >= 0, "Negative input inside `isqrt`.");
            $UnsignedT(n as $UnsignedT) as $SignedT
        }
    };
}

signed_fn!(i8, u8);
signed_fn!(i16, u16);
signed_fn!(i32, u32);
signed_fn!(i64, u64);
signed_fn!(i128, u128);

/// 生成一个 `u*` 函数，返回某个无符号整数类型任意输入的
/// [integer square root](https://en.wikipedia.org/wiki/Integer_square_root)。
macro_rules! unsigned_fn {
    ($UnsignedT:ident, $HalfBitsT:ident, $stages:ident) => {
        /// 返回任意
        #[doc = concat!("[`", stringify!($UnsignedT), "`](prim@", stringify!($UnsignedT), ")")]
        /// input.
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub(super) const fn $UnsignedT(mut n: $UnsignedT) -> $UnsignedT {
            if n <= <$HalfBitsT>::MAX as $UnsignedT {
                $HalfBitsT(n as $HalfBitsT) as $UnsignedT
            } else {
                // 规格化位移满足 Karatsuba 平方根算法的前置条件 “a₃ ≥ b/4”：a₃ 是 `n`
                // 最高四分之一位组成的块，b 是该四分之一位宽能表示的值数量。
                //
                // 因此 b/4 在二进制里就是除第二高位为 1 外其余全 0 的形态 (010...0)。
                // 既然 a₃ 至少是 b/4，a₃ 的最高位或相邻位必有一个 1；而 a₃ 的最高位正是
                // `n` 的最高位，所以 `n` 也满足同样性质。
                //
                // 选择偶数位移，是因为偶数位移会让平方根只左移规格化位移的一半：
                //
                // sqrt(n << (2 * p))
                // sqrt(2.pow(2 * p) * n)
                // sqrt(2.pow(2 * p)) * sqrt(n)
                // 2.pow(p) * sqrt(n)
                // sqrt(n) << p
                //
                // 如果移动奇数位，则会多出一个不适合整数算法处理的 sqrt(2) 因子：
                //
                // sqrt(n << (2 * p + 1))
                // sqrt(2.pow(2 * p + 1) * n)
                // sqrt(2 * 2.pow(2 * p) * n)
                // sqrt(2) * sqrt(2.pow(2 * p)) * sqrt(n)
                // sqrt(2) * 2.pow(p) * sqrt(n)
                // sqrt(2) * (sqrt(n) << p)
                const EVEN_MAKING_BITMASK: u32 = !1;
                let normalization_shift = n.leading_zeros() & EVEN_MAKING_BITMASK;
                n <<= normalization_shift;

                let s = $stages(n);

                let denormalization_shift = normalization_shift >> 1;
                s >> denormalization_shift
            }
        }
    };
}

/// 生成规格化后的第一阶段计算。
///
/// # 安全性(Safety）
///
/// `$n` 必须非零。
macro_rules! first_stage {
    ($original_bits:literal, $n:ident) => {{
        debug_assert!($n != 0, "`$n` is  zero in `first_stage!`.");

        const N_SHIFT: u32 = $original_bits - 8;
        let n = $n >> N_SHIFT;

        let (s, r) = U8_ISQRT_WITH_REMAINDER[n as usize];

        // 告诉优化器 `s` 非零。这样下一阶段用它作除数时，编译器无需生成处理除零 panic
        // 的代码。
        //
        // SAFETY: 如果原始 `$n` 为零，`unsigned_fn` 宏开头会递归到更小类型路径，而不会
        // 走到这里；因此能到达此处时，原始 `$n` 一定不是 0。
        //
        // 随后 `unsigned_fn` 宏会规格化 `$n`，保证最高两个有效位中至少有一个是 1。
        //
        // 本阶段再把 `$n` 的最高 8 位放入 `n`。这意味着这里的 `n` 在最高两位中至少有
        // 一个 1，因此 `n` 非零。
        //
        // `U8_ISQRT_WITH_REMAINDER[n as usize]` 对非零 `n` 会给出非零的 `s`。
        unsafe { crate::hint::assert_unchecked(s != 0) };
        (s, r)
    }};
}

/// 生成中间阶段计算。
///
/// # 安全性(Safety）
///
/// `$s` 必须非零。
macro_rules! middle_stage {
    ($original_bits:literal, $ty:ty, $n:ident, $s:ident, $r:ident) => {{
        debug_assert!($s != 0, "`$s` is  zero in `middle_stage!`.");

        const N_SHIFT: u32 = $original_bits - <$ty>::BITS;
        let n = ($n >> N_SHIFT) as $ty;

        const HALF_BITS: u32 = <$ty>::BITS >> 1;
        const QUARTER_BITS: u32 = <$ty>::BITS >> 2;
        const LOWER_HALF_1_BITS: $ty = (1 << HALF_BITS) - 1;
        const LOWEST_QUARTER_1_BITS: $ty = (1 << QUARTER_BITS) - 1;

        let lo = n & LOWER_HALF_1_BITS;
        let numerator = (($r as $ty) << QUARTER_BITS) | (lo >> QUARTER_BITS);
        let denominator = ($s as $ty) << 1;
        let q = numerator / denominator;
        let u = numerator % denominator;

        let mut s = ($s << QUARTER_BITS) as $ty + q;
        let (mut r, overflow) =
            ((u << QUARTER_BITS) | (lo & LOWEST_QUARTER_1_BITS)).overflowing_sub(q * q);
        if overflow {
            r = r.wrapping_add(2 * s - 1);
            s -= 1;
        }

        // 告诉优化器 `s` 非零。这样下一阶段用它作除数时，编译器无需生成处理除零 panic
        // 的代码。
        //
        // SAFETY: 如果原始 `$n` 为零，`unsigned_fn` 宏开头会递归到更小类型路径，而不会
        // 走到这里；因此能到达此处时，原始 `$n` 一定不是 0。
        //
        // 随后 `unsigned_fn` 宏会规格化 `$n`，保证最高两个有效位中至少有一个是 1。
        //
        // 随后各阶段会取出 `$n` 中能放入本阶段类型的若干最高位。例如处理 `u32` 的阶段
        // 会处理 `$n` 的最高 32 位。这保证每个阶段的 `n` 在最高两位中至少有一个 1，
        // 因而 `n` 非零。
        //
        // 本阶段会为该 `n` 计算正确的整数平方根；既然 `n` 非零，得到的 `s` 也非零。
        unsafe { crate::hint::assert_unchecked(s != 0) };
        (s, r)
    }};
}

/// 生成反规格化之前的最后阶段计算。
///
/// # 安全性(Safety）
///
/// `$s` 必须非零。
macro_rules! last_stage {
    ($ty:ty, $n:ident, $s:ident, $r:ident) => {{
        debug_assert!($s != 0, "`$s` is  zero in `last_stage!`.");

        const HALF_BITS: u32 = <$ty>::BITS >> 1;
        const QUARTER_BITS: u32 = <$ty>::BITS >> 2;
        const LOWER_HALF_1_BITS: $ty = (1 << HALF_BITS) - 1;

        let lo = $n & LOWER_HALF_1_BITS;
        let numerator = (($r as $ty) << QUARTER_BITS) | (lo >> QUARTER_BITS);
        let denominator = ($s as $ty) << 1;

        let q = numerator / denominator;
        let mut s = ($s << QUARTER_BITS) as $ty + q;
        let (s_squared, overflow) = s.overflowing_mul(s);
        if overflow || s_squared > $n {
            s -= 1;
        }
        s
    }};
}

/// 接收规格化后的 [`u16`](prim@u16) 输入，并返回规格化尺度下的
/// [integer square root](https://en.wikipedia.org/wiki/Integer_square_root)。
///
/// # 安全性(Safety）
///
/// `n` 必须非零。
#[inline]
const fn u16_stages(n: u16) -> u16 {
    let (s, r) = first_stage!(16, n);
    last_stage!(u16, n, s, r)
}

/// 接收规格化后的 [`u32`](prim@u32) 输入，并返回规格化尺度下的
/// [integer square root](https://en.wikipedia.org/wiki/Integer_square_root)。
///
/// # 安全性(Safety）
///
/// `n` 必须非零。
#[inline]
const fn u32_stages(n: u32) -> u32 {
    let (s, r) = first_stage!(32, n);
    let (s, r) = middle_stage!(32, u16, n, s, r);
    last_stage!(u32, n, s, r)
}

/// 接收规格化后的 [`u64`](prim@u64) 输入，并返回规格化尺度下的
/// [integer square root](https://en.wikipedia.org/wiki/Integer_square_root)。
///
/// # 安全性(Safety）
///
/// `n` 必须非零。
#[inline]
const fn u64_stages(n: u64) -> u64 {
    let (s, r) = first_stage!(64, n);
    let (s, r) = middle_stage!(64, u16, n, s, r);
    let (s, r) = middle_stage!(64, u32, n, s, r);
    last_stage!(u64, n, s, r)
}

/// 接收规格化后的 [`u128`](prim@u128) 输入，并返回规格化尺度下的
/// [integer square root](https://en.wikipedia.org/wiki/Integer_square_root)。
///
/// # 安全性(Safety）
///
/// `n` 必须非零。
#[inline]
const fn u128_stages(n: u128) -> u128 {
    let (s, r) = first_stage!(128, n);
    let (s, r) = middle_stage!(128, u16, n, s, r);
    let (s, r) = middle_stage!(128, u32, n, s, r);
    let (s, r) = middle_stage!(128, u64, n, s, r);
    last_stage!(u128, n, s, r)
}

unsigned_fn!(u16, u8, u16_stages);
unsigned_fn!(u32, u16, u32_stages);
unsigned_fn!(u64, u32, u64_stages);
unsigned_fn!(u128, u64, u128_stages);

/// 只实例化一次 `isqrt` 的负数 panic 逻辑，避免每个原始整数类型上的方法都生成一份。
#[cold]
#[track_caller]
pub(super) const fn panic_for_negative_argument() -> ! {
    panic!("argument of integer square root cannot be negative")
}
