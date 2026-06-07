//! "Printing Floating-Point Numbers Quickly and Accurately with Integers"[^1] 中 Grisu3
//! algorithm 的 Rust 改写版。它使用约 1KB 的预计算表，换取大多数输入上的高速路径。
//!
//! [^1]: Florian Loitsch. 2010. Printing floating-point numbers quickly and
//!   accurately with integers. SIGPLAN Not. 45, 6 (June 2010), 233-243.

use crate::mem::MaybeUninit;
use crate::num::diy_float::Fp;
use crate::num::flt2dec::{Decoded, MAX_SIG_DIGITS, round_up};

// 取值理由见 `format_shortest_opt` 中的注释。
#[doc(hidden)]
pub const ALPHA: i16 = -60;
#[doc(hidden)]
pub const GAMMA: i16 = -32;

/*
# 下列 Python 代码生成这张表：
for i in xrange(-308, 333, 8):
    if i >= 0: f = 10**i; e = 0
    else: f = 2**(80-4*i) // 10**-i; e = 4 * i - 80
    l = f.bit_length()
    f = ((f << 64 >> (l-1)) + 1) >> 1; e += l - 64
    print '    (%#018x, %5d, %4d),' % (f, e, i)
*/

#[doc(hidden)]
pub static CACHED_POW10: [(u64, i16, i16); 81] = [
    // (f, e, k)
    (0xe61acf033d1a45df, -1087, -308),
    (0xab70fe17c79ac6ca, -1060, -300),
    (0xff77b1fcbebcdc4f, -1034, -292),
    (0xbe5691ef416bd60c, -1007, -284),
    (0x8dd01fad907ffc3c, -980, -276),
    (0xd3515c2831559a83, -954, -268),
    (0x9d71ac8fada6c9b5, -927, -260),
    (0xea9c227723ee8bcb, -901, -252),
    (0xaecc49914078536d, -874, -244),
    (0x823c12795db6ce57, -847, -236),
    (0xc21094364dfb5637, -821, -228),
    (0x9096ea6f3848984f, -794, -220),
    (0xd77485cb25823ac7, -768, -212),
    (0xa086cfcd97bf97f4, -741, -204),
    (0xef340a98172aace5, -715, -196),
    (0xb23867fb2a35b28e, -688, -188),
    (0x84c8d4dfd2c63f3b, -661, -180),
    (0xc5dd44271ad3cdba, -635, -172),
    (0x936b9fcebb25c996, -608, -164),
    (0xdbac6c247d62a584, -582, -156),
    (0xa3ab66580d5fdaf6, -555, -148),
    (0xf3e2f893dec3f126, -529, -140),
    (0xb5b5ada8aaff80b8, -502, -132),
    (0x87625f056c7c4a8b, -475, -124),
    (0xc9bcff6034c13053, -449, -116),
    (0x964e858c91ba2655, -422, -108),
    (0xdff9772470297ebd, -396, -100),
    (0xa6dfbd9fb8e5b88f, -369, -92),
    (0xf8a95fcf88747d94, -343, -84),
    (0xb94470938fa89bcf, -316, -76),
    (0x8a08f0f8bf0f156b, -289, -68),
    (0xcdb02555653131b6, -263, -60),
    (0x993fe2c6d07b7fac, -236, -52),
    (0xe45c10c42a2b3b06, -210, -44),
    (0xaa242499697392d3, -183, -36),
    (0xfd87b5f28300ca0e, -157, -28),
    (0xbce5086492111aeb, -130, -20),
    (0x8cbccc096f5088cc, -103, -12),
    (0xd1b71758e219652c, -77, -4),
    (0x9c40000000000000, -50, 4),
    (0xe8d4a51000000000, -24, 12),
    (0xad78ebc5ac620000, 3, 20),
    (0x813f3978f8940984, 30, 28),
    (0xc097ce7bc90715b3, 56, 36),
    (0x8f7e32ce7bea5c70, 83, 44),
    (0xd5d238a4abe98068, 109, 52),
    (0x9f4f2726179a2245, 136, 60),
    (0xed63a231d4c4fb27, 162, 68),
    (0xb0de65388cc8ada8, 189, 76),
    (0x83c7088e1aab65db, 216, 84),
    (0xc45d1df942711d9a, 242, 92),
    (0x924d692ca61be758, 269, 100),
    (0xda01ee641a708dea, 295, 108),
    (0xa26da3999aef774a, 322, 116),
    (0xf209787bb47d6b85, 348, 124),
    (0xb454e4a179dd1877, 375, 132),
    (0x865b86925b9bc5c2, 402, 140),
    (0xc83553c5c8965d3d, 428, 148),
    (0x952ab45cfa97a0b3, 455, 156),
    (0xde469fbd99a05fe3, 481, 164),
    (0xa59bc234db398c25, 508, 172),
    (0xf6c69a72a3989f5c, 534, 180),
    (0xb7dcbf5354e9bece, 561, 188),
    (0x88fcf317f22241e2, 588, 196),
    (0xcc20ce9bd35c78a5, 614, 204),
    (0x98165af37b2153df, 641, 212),
    (0xe2a0b5dc971f303a, 667, 220),
    (0xa8d9d1535ce3b396, 694, 228),
    (0xfb9b7cd9a4a7443c, 720, 236),
    (0xbb764c4ca7a44410, 747, 244),
    (0x8bab8eefb6409c1a, 774, 252),
    (0xd01fef10a657842c, 800, 260),
    (0x9b10a4e5e9913129, 827, 268),
    (0xe7109bfba19c0c9d, 853, 276),
    (0xac2820d9623bf429, 880, 284),
    (0x80444b5e7aa7cf85, 907, 292),
    (0xbf21e44003acdd2d, 933, 300),
    (0x8e679c2f5e44ff8f, 960, 308),
    (0xd433179d9c8cb841, 986, 316),
    (0x9e19db92b4e31ba9, 1013, 324),
    (0xeb96bf6ebadf77d9, 1039, 332),
];

#[doc(hidden)]
pub const CACHED_POW10_FIRST_E: i16 = -1087;
#[doc(hidden)]
pub const CACHED_POW10_LAST_E: i16 = 1039;

#[doc(hidden)]
pub fn cached_power(alpha: i16, gamma: i16) -> (i16, Fp) {
    let offset = CACHED_POW10_FIRST_E as i32;
    let range = (CACHED_POW10.len() as i32) - 1;
    let domain = (CACHED_POW10_LAST_E - CACHED_POW10_FIRST_E) as i32;
    let idx = ((gamma as i32) - offset) * range / domain;
    let (f, e, k) = CACHED_POW10[idx as usize];
    debug_assert!(alpha <= e && e <= gamma);
    (k, Fp { f, e })
}

/// 给定 `x > 0`，返回 `(k, 10^k)`，满足 `10^k <= x < 10^(k + 1)`。
#[doc(hidden)]
pub fn max_pow10_no_more_than(x: u32) -> (u8, u32) {
    debug_assert!(x > 0);

    const X9: u32 = 10_0000_0000;
    const X8: u32 = 1_0000_0000;
    const X7: u32 = 1000_0000;
    const X6: u32 = 100_0000;
    const X5: u32 = 10_0000;
    const X4: u32 = 1_0000;
    const X3: u32 = 1000;
    const X2: u32 = 100;
    const X1: u32 = 10;

    if x < X4 {
        if x < X2 {
            if x < X1 { (0, 1) } else { (1, X1) }
        } else {
            if x < X3 { (2, X2) } else { (3, X3) }
        }
    } else {
        if x < X6 {
            if x < X5 { (4, X4) } else { (5, X5) }
        } else if x < X8 {
            if x < X7 { (6, X6) } else { (7, X7) }
        } else {
            if x < X9 { (8, X8) } else { (9, X9) }
        }
    }
}

/// Grisu 的 shortest mode 实现。
///
/// 如果无法保证返回精确表示，则返回 `None`，由调用方回退到 Dragon。
pub fn format_shortest_opt<'a>(
    d: &Decoded,
    buf: &'a mut [MaybeUninit<u8>],
) -> Option<(/*digits*/ &'a [u8], /*exp*/ i16)> {
    assert!(d.mant > 0);
    assert!(d.minus > 0);
    assert!(d.plus > 0);
    assert!(d.mant.checked_add(d.plus).is_some());
    assert!(d.mant.checked_sub(d.minus).is_some());
    assert!(buf.len() >= MAX_SIG_DIGITS);
    assert!(d.mant + d.plus < (1 << 61)); // 至少需要额外 3 位精度。

    // 从具有共享 exponent 的规格化值开始。
    let plus = Fp { f: d.mant + d.plus, e: d.exp }.normalize();
    let minus = Fp { f: d.mant - d.minus, e: d.exp }.normalize_to(plus.e);
    let v = Fp { f: d.mant, e: d.exp }.normalize_to(plus.e);

    // 找到某个 `cached = 10^minusk`，使 `ALPHA <= minusk + plus.e + 64 <= GAMMA`。
    // 因为 `plus` 已规格化，这意味着
    // `2^(62 + ALPHA) <= plus * cached < 2^(64 + GAMMA)`；按这里选择的 `ALPHA` 和
    // `GAMMA`，`plus * cached` 会落入 `[4, 2^32)`。
    //
    // 显然希望最大化 `GAMMA - ALPHA`，这样不需要太多缓存的 10 的幂；但还要考虑：
    //
    // 1. 希望 `floor(plus * cached)` 保持在 `u32` 内，因为它需要一次昂贵的除法。
    //    这基本无法避免，因为精度估算需要余数。
    // 2. `floor(plus * cached)` 的余数会反复乘以 10，不能溢出。
    //
    // 第一个条件给出 `64 + GAMMA <= 32`，第二个条件给出 `10 * 2^-ALPHA <= 2^64`；
    // 在这些约束下，-60 和 -32 是最大范围，V8 也使用这组值。
    let (minusk, cached) = cached_power(ALPHA - plus.e - 64, GAMMA - plus.e - 64);

    // 缩放 Fp。根据 Theorem 5.1，最大误差为 1 ulp。
    let plus = plus.mul(cached);
    let minus = minus.mul(cached);
    let v = v.mul(cached);
    debug_assert_eq!(plus.e, minus.e);
    debug_assert_eq!(plus.e, v.e);

    //         +- actual range of minus
    //   | <---|---------------------- unsafe region --------------------------> |
    //   |     |                                                                 |
    //   |  |<--->|  | <--------------- safe region ---------------> |           |
    //   |  |     |  |                                               |           |
    //   |1 ulp|1 ulp|                 |1 ulp|1 ulp|                 |1 ulp|1 ulp|
    //   |<--->|<--->|                 |<--->|<--->|                 |<--->|<--->|
    //   |-----|-----|-------...-------|-----|-----|-------...-------|-----|-----|
    //   |   minus   |                 |     v     |                 |   plus    |
    // minus1     minus0           v - 1 ulp   v + 1 ulp           plus0       plus1
    //
    // 上图中的 `minus`、`v` 和 `plus` 都是*量化*近似（误差 < 1 ulp）。由于不知道误差为正
    // 还是为负，因此使用两个等距近似，最大误差为 2 ulps。
    //
    // "unsafe region" 是初始生成时采用的宽松区间；"safe region" 是最终只接受的保守区间。
    // 我们先在 unsafe region 内得到一个正确表示，再尝试找到同时位于 safe region 且最接近
    // `v` 的表示。如果找不到，就放弃并回退。
    let plus1 = plus.f + 1;
    //  let plus0 = plus.f - 1; // 仅用于解释。
    //  let minus0 = minus.f + 1; // 仅用于解释。
    let minus1 = minus.f - 1;
    let e = -plus.e as usize; // 共享 exponent。

    // 把 `plus1` 拆成整数部分和小数部分。整数部分保证能放入 u32，因为缓存幂保证
    // `plus < 2^32`，且受精度要求约束，规格化后的 `plus.f` 始终小于 `2^64 - 2^4`。
    let plus1int = (plus1 >> e) as u32;
    let plus1frac = plus1 & ((1 << e) - 1);

    // 计算不大于 `plus1` 的最大 `10^max_kappa`（因此 `plus1 < 10^(max_kappa + 1)`）。
    // 这是下面 `kappa` 的上界。
    let (max_kappa, max_ten_kappa) = max_pow10_no_more_than(plus1int);

    let mut i = 0;
    let exp = max_kappa as i16 - minusk + 1;

    // Theorem 6.2：若 `k` 是满足 `0 <= y mod 10^k <= y - x` 的最大整数，则
    // `V = floor(y / 10^k) * 10^k` 位于 `[x, y]` 内，并且是该范围内的 shortest 表示之一
    // （有效数字数量最少）。
    //
    // 按 Theorem 6.2，在 `(minus1, plus1)` 之间寻找数字长度 `kappa`。通过要求
    // `y mod 10^k < y - x`，可把定理改造成排除 `x` 的形式。
    // 例如 `x = 32000`、`y = 32777` 时，`kappa = 2`，因为
    // `y mod 10^3 = 777 < y - x = 777` 不成立。算法依赖后续验证阶段排除 `y`。
    let delta1 = plus1 - minus1;
    //  let delta1int = (delta1 >> e) as usize; // 仅用于解释。
    let delta1frac = delta1 & ((1 << e) - 1);

    // 渲染整数部分，同时在每一步检查精度。
    let mut ten_kappa = max_ten_kappa; // 10^kappa
    let mut remainder = plus1int; // digits yet to be rendered
    loop {
        // 因为 `plus1 >= 10^kappa`，始终至少有一位可渲染。
        // 不变量：
        // - `delta1int <= remainder < 10^(kappa+1)`
        // - `plus1int = d[0..n-1] * 10^(kappa+1) + remainder`
        //   (it follows that `remainder = plus1int % 10^(kappa+1)`)

        // 把 `remainder` 除以 `10^kappa`；两者都按 `2^-e` 缩放。
        let q = remainder / ten_kappa;
        let r = remainder % ten_kappa;
        debug_assert!(q < 10);
        buf[i] = MaybeUninit::new(b'0' + q as u8);
        i += 1;

        let plus1rem = ((r as u64) << e) + plus1frac; // == (plus1 % 10^kappa) * 2^e
        if plus1rem < delta1 {
            // `plus1 % 10^kappa < delta1 = plus1 - minus1`，已经找到正确的 `kappa`。
            let ten_kappa = (ten_kappa as u64) << e; // 把 10^kappa 缩放回共享 exponent。
            return round_and_weed(
                // SAFETY: 上面已经初始化了这段内存。
                unsafe { buf[..i].assume_init_mut() },
                exp,
                plus1rem,
                delta1,
                plus1 - v.f,
                ten_kappa,
                1,
            );
        }

        // 渲染完所有整数位时跳出。因为 `plus1 < 10^(max_kappa + 1)`，精确位数是
        // `max_kappa + 1`。
        if i > max_kappa as usize {
            debug_assert_eq!(ten_kappa, 1);
            break;
        }

        // 恢复不变量。
        ten_kappa /= 10;
        remainder = r;
    }

    // 渲染小数部分，同时在每一步检查精度。这次依赖反复乘法，因为除法会损失精度。
    let mut remainder = plus1frac;
    let mut threshold = delta1frac;
    let mut ulp = 1;
    loop {
        // 前面跳出前已经检查过，下一位应为有效数字。
        // 不变量，其中 `m = max_kappa + 1`（整数部分位数）：
        // - `remainder < 2^e`
        // - `plus1frac * 10^(n-m) = d[m..n-1] * 2^e + remainder`

        remainder *= 10; // 不会溢出，`2^e * 10 < 2^64`。
        threshold *= 10;
        ulp *= 10;

        // 把 `remainder` 除以 `10^kappa`。两者都按 `2^e / 10^kappa` 缩放，
        // 因而后者在这里是隐含的。
        let q = remainder >> e;
        let r = remainder & ((1 << e) - 1);
        debug_assert!(q < 10);
        buf[i] = MaybeUninit::new(b'0' + q as u8);
        i += 1;

        if r < threshold {
            let ten_kappa = 1 << e; // implicit divisor
            return round_and_weed(
                // SAFETY: 上面已经初始化了这段内存。
                unsafe { buf[..i].assume_init_mut() },
                exp,
                r,
                threshold,
                (plus1 - v.f) * ulp,
                ten_kappa,
                ulp,
            );
        }

        // 恢复不变量。
        remainder = r;
    }

    // 已经生成了 `plus1` 的所有有效数字，但还不确定它是否最优。例如 `minus1` 为
    // 3.14153... 而 `plus1` 为 3.14158... 时，从 3.14154 到 3.14158 共有 5 个不同的
    // shortest 表示，而当前只有最大的那个。必须逐步减小最后一位并检查是否最优。
    // 候选最多 9 个（..1 到 ..9），所以这一步很快。（"rounding" 阶段）
    //
    // 该函数还检查这个“最优”表示是否真的位于 ulp 范围内；另外由于舍入误差，
    // “次优”表示也可能才是真正最优。两种情况下都返回 `None`。（"weeding" 阶段）
    //
    // 这里所有参数都按共同（但隐含）的值 `k` 缩放，因此：
    // - `remainder = (plus1 % 10^kappa) * k`
    // - `threshold = (plus1 - minus1) * k` (and also, `remainder < threshold`)
    // - `plus1v = (plus1 - v) * k` (and also, `threshold > plus1v` from prior invariants)
    // - `ten_kappa = 10^kappa * k`
    // - `ulp = 2^-e * k`
    fn round_and_weed(
        buf: &mut [u8],
        exp: i16,
        remainder: u64,
        threshold: u64,
        plus1v: u64,
        ten_kappa: u64,
        ulp: u64,
    ) -> Option<(&[u8], i16)> {
        assert!(!buf.is_empty());

        // 生成两个与 `v`（实际是 `plus1 - v`）相差 1.5 ulps 以内的近似。
        // 最终表示应同时最接近二者。
        //
        // 这里使用 `plus1 - v`，因为为了避免溢出/下溢，计算都以 `plus1` 为参照
        // （所以名称看起来有些反过来）。
        let plus1v_down = plus1v + ulp; // plus1 - (v - 1 ulp)
        let plus1v_up = plus1v - ulp; // plus1 - (v + 1 ulp)

        // 递减最后一位，并在最接近 `v + 1 ulp` 的表示处停止。
        let mut plus1w = remainder; // plus1w(n) = plus1 - w(n)
        {
            let last = buf.last_mut().unwrap();

            // 使用近似数字 `w(n)`：初始时它等于 `plus1 - plus1 % 10^kappa`。循环体执行
            // `n` 次后，`w(n) = plus1 - plus1 % 10^kappa - n * 10^kappa`。为了简化检查，
            // 令 `plus1w(n) = plus1 - w(n) = plus1 % 10^kappa + n * 10^kappa`
            // （因此 `remainder = plus1w(0)`）。注意 `plus1w(n)` 始终递增。
            //
            // 有三个终止条件。任意条件成立都会让循环无法继续；但此时至少已知有一个有效
            // 表示最接近 `v + 1 ulp`。为简洁起见，将它们记作 TC1 到 TC3。
            //
            // TC1：`w(n) <= v + 1 ulp`，也就是这是仍可能成为最近值的最后一个表示。
            // 它等价于 `plus1 - w(n) = plus1w(n) >= plus1 - (v + 1 ulp) = plus1v_up`。
            // 结合 TC2（检查 `w(n+1)` 是否有效）可以避免计算 `plus1w(n)` 时可能溢出。
            //
            // TC2：`w(n+1) < minus1`，也就是下一个表示一定不会舍入到 `v`。它等价于
            // `plus1 - w(n) + 10^kappa = plus1w(n) + 10^kappa >
            // plus1 - minus1 = threshold`。左侧可能溢出，但已知 `threshold > plus1v`；
            // 若 TC1 为假，则 `threshold - plus1w(n) >
            // threshold - (plus1v - 1 ulp) > 1 ulp`，因此可以安全地改测
            // `threshold - plus1w(n) < 10^kappa`。
            //
            // TC3：`abs(w(n) - (v + 1 ulp)) <= abs(w(n+1) - (v + 1 ulp))`，即下一个表示
            // 不比当前表示更接近 `v + 1 ulp`。令 `z(n) = plus1v_up - plus1w(n)`，则该条件
            // 变为 `abs(z(n)) <= abs(z(n+1))`。同样假设 TC1 为假，就有 `z(n) > 0`。
            // 需要考虑两种情况：
            //
            // - 当 `z(n+1) >= 0`：TC3 变成 `z(n) <= z(n+1)`。由于 `plus1w(n)` 递增，
            //   `z(n)` 应递减，所以这显然为假。
            // - 当 `z(n+1) < 0`：
            //   - TC3a：前置是 `plus1v_up < plus1w(n) + 10^kappa`。假设 TC2 为假，
            //     则 `threshold >= plus1w(n) + 10^kappa`，所以不会溢出。
            //   - TC3b：TC3 变成 `z(n) <= -z(n+1)`，即
            //     `plus1v_up - plus1w(n) >=
            //     plus1w(n+1) - plus1v_up = plus1w(n) + 10^kappa - plus1v_up`。
            //     由非 TC1 可得 `plus1v_up > plus1w(n)`，再结合 TC3a，不会溢出或下溢。
            //
            // 因此应在 `TC1 || TC2 || (TC3a && TC3b)` 时停止。下面条件等价于其反面：
            // `!TC1 && !TC2 && (!TC3a || !TC3b)`。
            while plus1w < plus1v_up
                && threshold - plus1w >= ten_kappa
                && (plus1w + ten_kappa < plus1v_up
                    || plus1v_up - plus1w >= plus1w + ten_kappa - plus1v_up)
            {
                *last -= 1;
                debug_assert!(*last > b'0'); // shortest 表示不能以 `0` 结尾。
                plus1w += ten_kappa;
            }
        }

        // 检查该表示是否也最接近 `v - 1 ulp`。
        //
        // 这与 `v + 1 ulp` 的终止条件相同，只是把所有 `plus1v_up` 换成 `plus1v_down`。
        // 溢出分析同样成立。
        if plus1w < plus1v_down
            && threshold - plus1w >= ten_kappa
            && (plus1w + ten_kappa < plus1v_down
                || plus1v_down - plus1w >= plus1w + ten_kappa - plus1v_down)
        {
            return None;
        }

        // 现在已经得到 `plus1` 与 `minus1` 之间最接近 `v` 的表示。不过该范围太宽松，
        // 因此要拒绝任何不在 `plus0` 和 `minus0` 之间的 `w(n)`，即
        // `plus1 - plus1w(n) <= minus0` 或 `plus1 - plus1w(n) >= plus0`。这里利用了
        // `threshold = plus1 - minus1` 以及 `plus1 - plus0 = minus0 - minus1 = 2 ulp`。
        if 2 * ulp <= plus1w && plus1w <= threshold - 4 * ulp { Some((buf, exp)) } else { None }
    }
}

/// 带 Dragon fallback 的 Grisu shortest mode 实现。
///
/// 大多数场景应使用该函数。
pub fn format_shortest<'a>(
    d: &Decoded,
    buf: &'a mut [MaybeUninit<u8>],
) -> (/*digits*/ &'a [u8], /*exp*/ i16) {
    use crate::num::flt2dec::strategy::dragon::format_shortest as fallback;
    // SAFETY: 借用检查器无法证明第二个分支中可以继续使用 `buf`，因此这里清洗生命周期。
    // 只有在 `format_shortest_opt` 返回 `None` 时才会重用 `buf`，所以不会同时存在两个
    // 有效可变借用。
    match format_shortest_opt(d, unsafe { &mut *(buf as *mut _) }) {
        Some(ret) => ret,
        None => fallback(d, buf),
    }
}

/// Grisu 的 exact 和 fixed mode 实现。
///
/// 如果无法保证返回精确表示，则返回 `None`，由调用方回退到 Dragon。
pub fn format_exact_opt<'a>(
    d: &Decoded,
    buf: &'a mut [MaybeUninit<u8>],
    limit: i16,
) -> Option<(/*digits*/ &'a [u8], /*exp*/ i16)> {
    assert!(d.mant > 0);
    assert!(d.mant < (1 << 61)); // 至少需要额外 3 位精度。
    assert!(!buf.is_empty());

    // 规格化并缩放 `v`。
    let v = Fp { f: d.mant, e: d.exp }.normalize();
    let (minusk, cached) = cached_power(ALPHA - v.e - 64, GAMMA - v.e - 64);
    let v = v.mul(cached);

    // 把 `v` 拆成整数部分和小数部分。
    let e = -v.e as usize;
    let vint = (v.f >> e) as u32;
    let vfrac = v.f & ((1 << e) - 1);

    let requested_digits = buf.len();

    const POW10_UP_TO_9: [u32; 10] =
        [1, 10, 100, 1000, 10_000, 100_000, 1_000_000, 10_000_000, 100_000_000, 1_000_000_000];

    // 这里偏离原始算法，先做若干早期检查以判断能否满足 `requested_digits`。如果确定不能，
    // 就提前退出，避免执行后面大部分重计算。
    //
    // 当 `vfrac` 为零时，可以轻松判断 `vint` 是否能满足请求的位数：
    //      若 `requested_digits >= 11`，`vint` 单独无法耗尽位数，因为
    //      `10^(11 - 1) > u32::MAX >= vint`。
    //      若 `vint < 10^(requested_digits - 1)`，`vint` 无法耗尽位数。
    //      否则 `vint` 可能可以耗尽位数，需要继续执行剩余代码。
    if (vfrac == 0) && ((requested_digits >= 11) || (vint < POW10_UP_TO_9[requested_digits - 1])) {
        return None;
    }

    // 旧 `v` 和新 `v`（按 `10^-k` 缩放）都有 < 1 ulp 的误差（Theorem 5.1）。由于不知道
    // 误差为正还是为负，因此使用两个等距近似，最大误差为 2 ulps（与 shortest 情况相同）。
    //
    // 目标是找到 `v - 1 ulp` 和 `v + 1 ulp` 共同拥有的、精确舍入后的数字序列，以获得最大
    // 置信度。如果做不到，就无法知道哪个才是 `v` 的正确输出，因此放弃并回退。
    //
    // 这里 `err` 定义为 `1 ulp * 2^e`（与 `vfrac` 中的 ulp 相同），并会随 `v` 缩放。
    let mut err = 1;

    // 计算不大于 `v` 的最大 `10^max_kappa`（因此 `v < 10^(max_kappa + 1)`）。
    // 这是下面 `kappa` 的上界。
    let (max_kappa, max_ten_kappa) = max_pow10_no_more_than(vint);

    let mut i = 0;
    let exp = max_kappa as i16 - minusk + 1;

    // 如果存在最后一位限制，需要在实际渲染前缩短缓冲区以避免 double rounding。
    // 注意发生向上舍入时必须再次扩展缓冲区。
    let len = if exp <= limit {
        // 连一位数字都无法生成。例如输入类似 9.5 且需要舍入到 10 时会发生这种情况。
        //
        // 原则上可以立即用空缓冲区调用 `possibly_round`，但把 `max_ten_kappa << e` 再乘
        // 10 可能溢出。因此这里保守处理，把误差范围扩大 10 倍。这会增加假阴性率，
        // 但幅度非常、非常小；只有当 mantissa 大于 60 位时才可能明显。
        //
        // SAFETY: `len = 0`，因此“前 len 个字节已初始化”的义务是平凡成立的。
        return unsafe {
            possibly_round(buf, 0, exp, limit, v.f / 10, (max_ten_kappa as u64) << e, err << e)
        };
    } else if ((exp as i32 - limit as i32) as usize) < buf.len() {
        (exp - limit) as usize
    } else {
        buf.len()
    };
    debug_assert!(len > 0);

    // 渲染整数部分。误差完全位于小数部分，因此这里无需检查。
    let mut kappa = max_kappa as i16;
    let mut ten_kappa = max_ten_kappa; // 10^kappa
    let mut remainder = vint; // digits yet to be rendered
    loop {
        // 始终至少有一位可渲染。
        // 不变量：
        // - `remainder < 10^(kappa+1)`
        // - `vint = d[0..n-1] * 10^(kappa+1) + remainder`
        //   （由此可得 `remainder = vint % 10^(kappa + 1)`）

        // 把 `remainder` 除以 `10^kappa`；两者都按 `2^-e` 缩放。
        let q = remainder / ten_kappa;
        let r = remainder % ten_kappa;
        debug_assert!(q < 10);
        buf[i] = MaybeUninit::new(b'0' + q as u8);
        i += 1;

        // 缓冲区是否已满？若已满，用余数执行舍入阶段。
        if i == len {
            let vrem = ((r as u64) << e) + vfrac; // == (v % 10^kappa) * 2^e
            // SAFETY: 已经初始化了 `len` 个字节。
            return unsafe {
                possibly_round(buf, len, exp, limit, vrem, (ten_kappa as u64) << e, err << e)
            };
        }

        // 渲染完所有整数位时跳出。因为 `plus1 < 10^(max_kappa + 1)`，精确位数是
        // `max_kappa + 1`。
        if i > max_kappa as usize {
            debug_assert_eq!(ten_kappa, 1);
            debug_assert_eq!(kappa, 0);
            break;
        }

        // 恢复不变量。
        kappa -= 1;
        ten_kappa /= 10;
        remainder = r;
    }

    // 渲染小数部分。
    //
    // 原则上可以继续到最后一个可用数字，再检查准确性。但这里使用有限大小整数，因此需要
    // 某种准则检测溢出。V8 使用 `remainder > err`，当 `v - 1 ulp` 与 `v` 的前 `i`
    // 个有效数字不同时该条件会变假；不过它会拒绝太多本来有效的输入。
    //
    // 由于后续阶段有正确的溢出检测，这里改用更紧的准则：持续生成直到 `err` 超过
    // `10^kappa / 2`，此时 `v - 1 ulp` 与 `v + 1 ulp` 之间必定包含两个或更多舍入表示。
    // 这对应 `possibly_round` 中前两个比较条件。
    let mut remainder = vfrac;
    let maxerr = 1 << (e - 1);
    while err < maxerr {
        // 不变量，其中 `m = max_kappa + 1`（整数部分位数）：
        // - `remainder < 2^e`
        // - `vfrac * 10^(n-m) = d[m..n-1] * 2^e + remainder`
        // - `err = 10^(n-m)`

        remainder *= 10; // 不会溢出，`2^e * 10 < 2^64`。
        err *= 10; // 不会溢出，`err * 10 < 2^e * 5 < 2^64`。

        // 把 `remainder` 除以 `10^kappa`。两者都按 `2^e / 10^kappa` 缩放，
        // 因而后者在这里是隐含的。
        let q = remainder >> e;
        let r = remainder & ((1 << e) - 1);
        debug_assert!(q < 10);
        buf[i] = MaybeUninit::new(b'0' + q as u8);
        i += 1;

        // 缓冲区是否已满？若已满，用余数执行舍入阶段。
        if i == len {
            // SAFETY: 已经初始化了 `len` 个字节。
            return unsafe { possibly_round(buf, len, exp, limit, r, 1 << e, err) };
        }

        // 恢复不变量。
        remainder = r;
    }

    // 继续计算已经无用（`possibly_round` 一定失败），因此放弃。
    return None;

    // 已生成 `v` 的所有请求数字，它们也应与 `v - 1 ulp` 的对应数字相同。现在检查是否存在
    // 一个同时被 `v - 1 ulp` 与 `v + 1 ulp` 共享的唯一表示；它可以与已生成数字相同，
    // 也可以是这些数字向上舍入后的版本。如果该范围包含多个相同长度的表示，则无法确定，
    // 应返回 `None`。
    //
    // 这里所有参数都按共同（但隐含）的值 `k` 缩放，因此：
    // - `remainder = (v % 10^kappa) * k`
    // - `ten_kappa = 10^kappa * k`
    // - `ulp = 2^-e * k`
    //
    // SAFETY: `buf` 的前 `len` 个字节必须已经初始化。
    unsafe fn possibly_round(
        buf: &mut [MaybeUninit<u8>],
        mut len: usize,
        mut exp: i16,
        limit: i16,
        remainder: u64,
        ten_kappa: u64,
        ulp: u64,
    ) -> Option<(&[u8], i16)> {
        debug_assert!(remainder < ten_kappa);

        //           10^kappa
        //    :   :   :<->:   :
        //    :   :   :   :   :
        //    :|1 ulp|1 ulp|  :
        //    :|<--->|<--->|  :
        // ----|-----|-----|----
        //     |     v     |
        // v - 1 ulp   v + 1 ulp
        //
        // （供参考：虚线表示给定位数下可能表示的精确值。）
        //
        // 误差太大，导致 `v - 1 ulp` 与 `v + 1 ulp` 之间至少有三个可能表示。
        // 因而无法确定哪一个正确。
        if ulp >= ten_kappa {
            return None;
        }

        //    10^kappa
        //   :<------->:
        //   :         :
        //   : |1 ulp|1 ulp|
        //   : |<--->|<--->|
        // ----|-----|-----|----
        //     |     v     |
        // v - 1 ulp   v + 1 ulp
        //
        // 实际上，1/2 ulp 已足以引入两个可能表示。（注意我们需要一个对 `v - 1 ulp` 和
        // `v + 1 ulp` 都唯一的表示。）这不会溢出，因为第一个检查已保证 `ulp < ten_kappa`。
        if ten_kappa - ulp <= ulp {
            return None;
        }

        //     remainder
        //       :<->|                           :
        //       :   |                           :
        //       :<--------- 10^kappa ---------->:
        //     | :   |                           :
        //     |1 ulp|1 ulp|                     :
        //     |<--->|<--->|                     :
        // ----|-----|-----|------------------------
        //     |     v     |
        // v - 1 ulp   v + 1 ulp
        //
        // 如果 `v + 1 ulp` 更接近向下舍入表示（也就是当前 `buf` 中的表示），就可以安全返回。
        // 注意 `v - 1 ulp` **可能**小于当前表示，但由于 `1 ulp < 10^kappa / 2`，
        // 该条件已经足够：`v - 1 ulp` 与当前表示之间的距离不会超过 `10^kappa / 2`。
        //
        // 该条件等价于 `remainder + ulp < 10^kappa / 2`。由于这很容易溢出，先检查
        // `remainder < 10^kappa / 2`。我们已经验证 `ulp < 10^kappa / 2`，因此只要
        // `10^kappa` 最终没有溢出，第二个检查就是安全的。
        if ten_kappa - remainder > remainder && ten_kappa - 2 * remainder >= 2 * ulp {
            // SAFETY: 调用方已经初始化了这段内存。
            return Some((unsafe { buf[..len].assume_init_ref() }, exp));
        }

        //   :<------- remainder ------>|   :
        //   :                          |   :
        //   :<--------- 10^kappa --------->:
        //   :                    |     |   : |
        //   :                    |1 ulp|1 ulp|
        //   :                    |<--->|<--->|
        // -----------------------|-----|-----|-----
        //                        |     v     |
        //                    v - 1 ulp   v + 1 ulp
        //
        // 另一方面，如果 `v - 1 ulp` 更接近向上舍入表示，就应向上舍入并返回。
        // 出于同样原因，不需要再检查 `v + 1 ulp`。
        //
        // 该条件等价于 `remainder - ulp >= 10^kappa / 2`。同样先检查 `remainder > ulp`
        // （注意不是 `remainder >= ulp`，因为 `10^kappa` 永不为零）。另请注意
        // `remainder - ulp <= 10^kappa`，所以第二个检查不会溢出。
        if remainder > ulp && ten_kappa - (remainder - ulp) <= remainder - ulp {
            if let Some(c) =
                // SAFETY: 调用方必须已经初始化这段内存。
                round_up(unsafe { buf[..len].assume_init_mut() })
            {
                // 只有请求 fixed precision 时才追加额外数字。还需要检查：如果原始缓冲区为空，
                // 只有 `exp == limit` 这个边界情况才允许追加额外数字。
                exp += 1;
                if exp > limit && len < buf.len() {
                    buf[len] = MaybeUninit::new(c);
                    len += 1;
                }
            }
            // SAFETY: 我们和调用方共同初始化了这段内存。
            return Some((unsafe { buf[..len].assume_init_ref() }, exp));
        }

        // 否则说明无法判定（`v - 1 ulp` 与 `v + 1 ulp` 之间有些值会向下舍入，另一些会
        // 向上舍入），只能放弃。
        None
    }
}

/// 带 Dragon fallback 的 Grisu exact 和 fixed mode 实现。
///
/// 大多数场景应使用该函数。
pub fn format_exact<'a>(
    d: &Decoded,
    buf: &'a mut [MaybeUninit<u8>],
    limit: i16,
) -> (/*digits*/ &'a [u8], /*exp*/ i16) {
    use crate::num::flt2dec::strategy::dragon::format_exact as fallback;
    // SAFETY: 借用检查器无法证明第二个分支中可以继续使用 `buf`，因此这里清洗生命周期。
    // 只有在 `format_exact_opt` 返回 `None` 时才会重用 `buf`，所以不会同时存在两个
    // 有效可变借用。
    match format_exact_opt(d, unsafe { &mut *(buf as *mut _) }, limit) {
        Some(ret) => ret,
        None => fallback(d, buf, limit),
    }
}
