//! "Printing Floating-Point Numbers Quickly and Accurately"[^1] 图 3 的近乎直接
//! Rust 翻译版，只做了少量优化。
//!
//! [^1]: Burger, R. G. and Dybvig, R. K. 1996. Printing floating-point numbers
//!   quickly and accurately. SIGPLAN Not. 31, 5 (May. 1996), 108-116.

use crate::cmp::Ordering;
use crate::mem::MaybeUninit;
use crate::num::bignum::{Big32x40 as Big, Digit32 as Digit};
use crate::num::flt2dec::estimator::estimate_scaling_factor;
use crate::num::flt2dec::{Decoded, MAX_SIG_DIGITS, round_up};

static POW10: [Digit; 10] =
    [1, 10, 100, 1000, 10000, 100000, 1000000, 10000000, 100000000, 1000000000];
// `5^(2^n)` 的 `Digit` 预计算数组。
static POW5TO16: [Digit; 2] = [0x86f26fc1, 0x23];
static POW5TO32: [Digit; 3] = [0x85acef81, 0x2d6d415b, 0x4ee];
static POW5TO64: [Digit; 5] = [0xbf6a1f01, 0x6e38ed64, 0xdaa797ed, 0xe93ff9f4, 0x184f03];
static POW5TO128: [Digit; 10] = [
    0x2e953e01, 0x3df9909, 0xf1538fd, 0x2374e42f, 0xd3cff5ec, 0xc404dc08, 0xbccdb0da, 0xa6337f19,
    0xe91f2603, 0x24e,
];
static POW5TO256: [Digit; 19] = [
    0x982e7c01, 0xbed3875b, 0xd8d99f72, 0x12152f87, 0x6bde50c6, 0xcf4a6e70, 0xd595d80f, 0x26b2716e,
    0xadc666b0, 0x1d153624, 0x3c42d35a, 0x63ff540e, 0xcc5573c0, 0x65f9ef17, 0x55bc28f2, 0x80dcc7f7,
    0xf46eeddc, 0x5fdcefce, 0x553f7,
];

#[doc(hidden)]
pub fn mul_pow10(x: &mut Big, n: usize) -> &mut Big {
    debug_assert!(n < 512);
    // 最小的若干情况直接乘 10 的幂，省掉后续左移。
    if n < 8 {
        return x.mul_small(POW10[n & 7]);
    }
    // 先乘 5 的幂，最后再把 2 的幂移入。这样中间乘积更小，也更快。
    if n & 7 != 0 {
        x.mul_small(POW10[n & 7] >> (n & 7));
    }
    if n & 8 != 0 {
        x.mul_small(POW10[8] >> 8);
    }
    if n & 16 != 0 {
        x.mul_digits(&POW5TO16);
    }
    if n & 32 != 0 {
        x.mul_digits(&POW5TO32);
    }
    if n & 64 != 0 {
        x.mul_digits(&POW5TO64);
    }
    if n & 128 != 0 {
        x.mul_digits(&POW5TO128);
    }
    if n & 256 != 0 {
        x.mul_digits(&POW5TO256);
    }
    x.mul_pow2(n)
}

fn div_2pow10(x: &mut Big, mut n: usize) -> &mut Big {
    let largest = POW10.len() - 1;
    while n > largest {
        x.div_rem_small(POW10[largest]);
        n -= largest;
    }
    x.div_rem_small(POW10[n] << 1);
    x
}

// 只可在 `x < 16 * scale` 时使用；`scaleN` 应是 `scale.mul_small(N)`。
fn div_rem_upto_16<'a>(
    x: &'a mut Big,
    scale: &Big,
    scale2: &Big,
    scale4: &Big,
    scale8: &Big,
) -> (u8, &'a mut Big) {
    let mut d = 0;
    if *x >= *scale8 {
        x.sub(scale8);
        d += 8;
    }
    if *x >= *scale4 {
        x.sub(scale4);
        d += 4;
    }
    if *x >= *scale2 {
        x.sub(scale2);
        d += 2;
    }
    if *x >= *scale {
        x.sub(scale);
        d += 1;
    }
    debug_assert!(*x < *scale);
    (d, x)
}

/// Dragon 的 shortest mode 实现。
pub fn format_shortest<'a>(
    d: &Decoded,
    buf: &'a mut [MaybeUninit<u8>],
) -> (/*digits*/ &'a [u8], /*exp*/ i16) {
    // 待格式化数 `v` 满足：
    // - 等于 `mant * 2^exp`；
    // - 在原类型中前一个可表示值是 `(mant - 2 * minus) * 2^exp`；
    // - 在原类型中后一个可表示值是 `(mant + 2 * plus) * 2^exp`。
    //
    // 显然 `minus` 和 `plus` 不能为零（Inf 会使用范围外值表示）。同时这里假设至少会生成
    // 一位数字，因此 `mant` 也不能为零。
    //
    // 这也意味着任何位于 `low = (mant - minus) * 2^exp` 和
    // `high = (mant + plus) * 2^exp` 之间的数，都会映射回这个精确浮点数；当原始 mantissa
    // 为偶数（即 `!mant_was_odd`）时包含边界。

    assert!(d.mant > 0);
    assert!(d.minus > 0);
    assert!(d.plus > 0);
    assert!(d.mant.checked_add(d.plus).is_some());
    assert!(d.mant.checked_sub(d.minus).is_some());
    assert!(buf.len() >= MAX_SIG_DIGITS);

    // `a.cmp(&b) < rounding` 等价于 `if d.inclusive { a <= b } else { a < b }`。
    let rounding = if d.inclusive { Ordering::Greater } else { Ordering::Equal };

    // 从原始输入估算满足 `10^(k_0 - 1) < high <= 10^(k_0 + 1)` 的 `k_0`。
    // 后续再计算满足 `10^(k - 1) < high <= 10^k` 的紧确边界 `k`。
    let mut k = estimate_scaling_factor(d.mant + d.plus, d.exp);

    // 把 `{mant, plus, minus} * 2^exp` 转为分数形式，使：
    // - `v = mant / scale`
    // - `low = (mant - minus) / scale`
    // - `high = (mant + plus) / scale`
    let mut mant = Big::from_u64(d.mant);
    let mut minus = Big::from_u64(d.minus);
    let mut plus = Big::from_u64(d.plus);
    let mut scale = Big::from_small(1);
    if d.exp < 0 {
        scale.mul_pow2(-d.exp as usize);
    } else {
        mant.mul_pow2(d.exp as usize);
        minus.mul_pow2(d.exp as usize);
        plus.mul_pow2(d.exp as usize);
    }

    // 把 `mant` 除以 `10^k`。现在有 `scale / 10 < mant + plus <= scale * 10`。
    if k >= 0 {
        mul_pow10(&mut scale, k as usize);
    } else {
        mul_pow10(&mut mant, -k as usize);
        mul_pow10(&mut minus, -k as usize);
        mul_pow10(&mut plus, -k as usize);
    }

    // 当 `mant + plus > scale`（或 `>=`）时修正。
    // 实际上不修改 `scale`，而是跳过初始乘法达到同样效果。
    // 现在 `scale < mant + plus <= scale * 10`，可以开始生成数字。
    //
    // 注意当 `scale - plus < mant < scale` 时，`d[0]` **可能**为零。
    // 这种情况下，下方的向上舍入条件(`up`)会立即触发。
    if scale.cmp(mant.clone().add(&plus)) < rounding {
        // 等价于把 `scale` 乘以 10。
        k += 1;
    } else {
        mant.mul_small(10);
        minus.mul_small(10);
        plus.mul_small(10);
    }

    // 缓存 `(2, 4, 8) * scale` 以便生成数字。
    let mut scale2 = scale.clone();
    scale2.mul_pow2(1);
    let mut scale4 = scale.clone();
    scale4.mul_pow2(2);
    let mut scale8 = scale.clone();
    scale8.mul_pow2(3);

    let mut down;
    let mut up;
    let mut i = 0;
    loop {
        // 不变量，其中 `d[0..n-1]` 是截至目前生成的数字：
        // - `v = mant / scale * 10^(k-n-1) + d[0..n-1] * 10^(k-n)`
        // - `v - low = minus / scale * 10^(k-n-1)`
        // - `high - v = plus / scale * 10^(k-n-1)`
        // - `(mant + plus) / scale <= 10` (thus `mant / scale < 10`)
        // 这里 `d[i..j]` 是 `d[i] * 10^(j-i) + ... + d[j-1] * 10 + d[j]` 的简写。

        // 生成一位：`d[n] = floor(mant / scale) < 10`。
        let (d, _) = div_rem_upto_16(&mut mant, &scale, &scale2, &scale4, &scale8);
        debug_assert!(d < 10);
        buf[i] = MaybeUninit::new(b'0' + d);
        i += 1;

        // 下面是 modified Dragon algorithm 的简化说明；为了便于阅读，省略了许多中间推导和
        // 完备性论证。
        //
        // 因为已经更新 `n`，先写出修改后的不变量：
        // - `v = mant / scale * 10^(k-n) + d[0..n-1] * 10^(k-n)`
        // - `v - low = minus / scale * 10^(k-n)`
        // - `high - v = plus / scale * 10^(k-n)`
        //
        // 假设 `d[0..n-1]` 是 `low` 与 `high` 之间的 shortest 表示，也就是
        // `d[0..n-1]` 满足以下两条，而 `d[0..n-2]` 不满足：
        // - `low < d[0..n-1] * 10^(k-n) < high`（双射性：这些数字能舍入回 `v`）；
        // - `abs(v / 10^(k-n) - d[0..n-1]) <= 1/2`（最后一位正确）。
        //
        // 第二个条件可化简为 `2 * mant <= scale`。把不变量按 `mant`、`low` 和 `high`
        // 求解，可得到第一个条件的简化版：`-plus < mant < minus`。由于
        // `-plus < 0 <= mant`，当 `mant < minus` 且 `2 * mant <= scale` 时，我们已经得到
        // 正确的 shortest 表示。（原始 mantissa 为偶数时，前者变为 `mant <= minus`。）
        //
        // 当第二个条件不成立（`2 * mant > scale`）时，需要增加最后一位。这足以恢复条件：
        // 数字生成过程已经保证 `0 <= v / 10^(k-n) - d[0..n-1] < 1`。此时第一个条件变为
        // `-plus < mant - scale < minus`。因为生成数字后有 `mant < scale`，所以得到
        // `scale < mant + plus`。（原始 mantissa 为偶数时同样变为 `scale <= mant + plus`。）
        //
        // 简而言之：
        // - 当 `mant < minus`（或 `<=`）时停止并向下舍入（保持数字不变）。
        // - 当 `scale < mant + plus`（或 `<=`）时停止并向上舍入（增加最后一位）。
        // - 否则继续生成数字。
        down = mant.cmp(&minus) < rounding;
        up = scale.cmp(mant.clone().add(&plus)) < rounding;
        if down || up {
            break;
        } // 已得到 shortest 表示，进入舍入阶段。

        // 恢复不变量。这样算法一定终止：`minus` 和 `plus` 始终增长，而 `mant` 会按
        // `scale` 取余裁剪，`scale` 保持固定。
        mant.mul_small(10);
        minus.mul_small(10);
        plus.mul_small(10);
    }

    // 向上舍入发生于：
    // i) 只有向上舍入条件被触发；
    // ii) 两个条件都被触发，且 tie-breaking 偏向向上。
    if up && (!down || *mant.mul_pow2(1) >= scale) {
        // 如果向上舍入改变了长度，exponent 也应改变。这个条件看起来极难满足（可能不可能），
        // 但这里保持保守和一致。
        // SAFETY: 上面已经初始化了这段内存。
        if let Some(c) = round_up(unsafe { buf[..i].assume_init_mut() }) {
            buf[i] = MaybeUninit::new(c);
            i += 1;
            k += 1;
        }
    }

    // SAFETY: 上面已经初始化了这段内存。
    (unsafe { buf[..i].assume_init_ref() }, k)
}

/// Dragon 的 exact 和 fixed mode 实现。
pub fn format_exact<'a>(
    d: &Decoded,
    buf: &'a mut [MaybeUninit<u8>],
    limit: i16,
) -> (/*digits*/ &'a [u8], /*exp*/ i16) {
    assert!(d.mant > 0);
    assert!(d.minus > 0);
    assert!(d.plus > 0);
    assert!(d.mant.checked_add(d.plus).is_some());
    assert!(d.mant.checked_sub(d.minus).is_some());

    // 从原始输入估算满足 `10^(k_0 - 1) < v <= 10^(k_0 + 1)` 的 `k_0`。
    let mut k = estimate_scaling_factor(d.mant, d.exp);

    // `v = mant / scale`。
    let mut mant = Big::from_u64(d.mant);
    let mut scale = Big::from_small(1);
    if d.exp < 0 {
        scale.mul_pow2(-d.exp as usize);
    } else {
        mant.mul_pow2(d.exp as usize);
    }

    // 把 `mant` 除以 `10^k`。现在有 `scale / 10 < mant <= scale * 10`。
    if k >= 0 {
        mul_pow10(&mut scale, k as usize);
    } else {
        mul_pow10(&mut mant, -k as usize);
    }

    // 当 `mant + plus >= scale` 时修正，其中 `plus / scale = 10^-buf.len() / 2`。
    // 为保持固定大小 bignum，实际使用的是 `mant + floor(plus) >= scale`。
    // 不真正修改 `scale`，而是跳过初始乘法达到同样效果。和 shortest 算法一样，
    // `d[0]` 可能为零，但最终会被向上舍入。
    if *div_2pow10(&mut scale.clone(), buf.len()).add(&mant) >= scale {
        // 等价于把 `scale` 乘以 10。
        k += 1;
    } else {
        mant.mul_small(10);
    }

    // 如果存在最后一位限制，需要在实际渲染前缩短缓冲区以避免 double rounding。
    // 注意发生向上舍入时必须再次扩展缓冲区。
    let mut len = if k < limit {
        // 连一位数字都无法生成。例如输入类似 9.5 且需要舍入到 10 时会发生这种情况。
        // 返回空缓冲区；例外是稍后的向上舍入路径，当 `k == limit` 时必须产生恰好一位数字。
        0
    } else if ((k as i32 - limit as i32) as usize) < buf.len() {
        (k - limit) as usize
    } else {
        buf.len()
    };

    if len > 0 {
        // 缓存 `(2, 4, 8) * scale` 以便生成数字。
        // 这可能较昂贵，因此缓冲区为空时不要计算它们。
        let mut scale2 = scale.clone();
        scale2.mul_pow2(1);
        let mut scale4 = scale.clone();
        scale4.mul_pow2(2);
        let mut scale8 = scale.clone();
        scale8.mul_pow2(3);

        for i in 0..len {
            if mant.is_zero() {
                // 后续数字全是零，在这里停止。不要尝试继续舍入，而是填满剩余数字。
                for c in &mut buf[i..len] {
                    *c = MaybeUninit::new(b'0');
                }
                // SAFETY: 上面已经初始化了这段内存。
                return (unsafe { buf[..len].assume_init_ref() }, k);
            }

            let mut d = 0;
            if mant >= scale8 {
                mant.sub(&scale8);
                d += 8;
            }
            if mant >= scale4 {
                mant.sub(&scale4);
                d += 4;
            }
            if mant >= scale2 {
                mant.sub(&scale2);
                d += 2;
            }
            if mant >= scale {
                mant.sub(&scale);
                d += 1;
            }
            debug_assert!(mant < scale);
            debug_assert!(d < 10);
            buf[i] = MaybeUninit::new(b'0' + d);
            mant.mul_small(10);
        }
    }

    // 如果在数字中间停止，需要考虑向上舍入。
    // 若后续数字恰好是 5000...，检查前一位并尝试 ties-to-even，也就是当前一位为偶数时
    // 避免向上舍入。
    let order = mant.cmp(scale.mul_small(5));
    if order == Ordering::Greater
        || (order == Ordering::Equal
            // SAFETY: `buf[len - 1]` 已初始化。
            && len > 0 && unsafe { buf[len - 1].assume_init() } & 1 == 1)
    {
        // 如果向上舍入改变了长度，exponent 也应改变。但调用方请求固定数字数量，因此不要
        // 直接改变缓冲区。
        // SAFETY: 上面已经初始化了这段内存。
        if let Some(c) = round_up(unsafe { buf[..len].assume_init_mut() }) {
            // 除非请求的是 fixed precision。还需要检查：若原始缓冲区为空，只有
            // `k == limit` 这个边界情况才允许追加额外数字。
            k += 1;
            if k > limit && len < buf.len() {
                buf[len] = MaybeUninit::new(c);
                len += 1;
            }
        }
    }

    // SAFETY: 上面已经初始化了这段内存。
    (unsafe { buf[..len].assume_init_ref() }, k)
}
