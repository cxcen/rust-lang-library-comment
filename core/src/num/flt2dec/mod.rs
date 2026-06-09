/*!

浮点数到十进制的转换例程。

# 问题陈述

给定浮点数 `v = f * 2^e`（其中 `f` 是整数），以及它的误差边界 `minus` 和 `plus`：
任意落在 `v - minus` 与 `v + plus` 之间的数都会舍入回 `v`。为简化说明，先把这个区间
视为开区间。我们希望得到唯一的十进制表示 `V = 0.d[0..n-1] * 10^k`，并满足：

- `d[0]` 非零。

- 回读时能正确舍入：`v - minus < V < v + plus`。并且它是满足该条件的最短表示，
  也就是说不存在少于 `n` 位数字的表示同样能正确舍入回 `v`。

- 它最接近原始值：`abs(V - v) <= 10^(k-n) / 2`。注意可能同时有两个表示满足这个
  唯一性要求，此时需要额外的 tie-breaking 机制。

我们把这种操作模式称为 *shortest* mode。没有额外约束时会使用该模式，也可以把它看作
“自然”模式，因为它符合通常直觉（至少会把 `0.1f32` 打印成 `"0.1"`）。

另外还有两个彼此密切相关的模式。在这些模式中，调用方会给出有效数字个数 `n`，或给出
最后一位位置限制 `limit`（由它决定实际 `n`）。目标仍是得到
`V = 0.d[0..n-1] * 10^k`，并满足：

- `d[0]` 非零；除非 `n` 为零，此时只返回 `k`。

- 它最接近原始值：`abs(V - v) <= 10^(k-n) / 2`。同样，必要时会使用 tie-breaking。

当给出 `limit` 但没有给出 `n` 时，会选择满足 `k - n = limit` 的 `n`，使最后一位
`d[n-1]` 的尺度为 `10^(k-n) = 10^limit`。如果得到的 `n` 为负，则把它截到零，
于是只返回 `k`。同时还受调用方提供的缓冲区限制；这种限制用于在事先不知道正确 `k`
的情况下，打印给定小数位数以内的数字。

需要 `n` 的模式称为 *exact* mode，需要 `limit` 的模式称为 *fixed* mode。exact mode 是
fixed mode 的子集：足够宽松的最后一位限制最终会填满调用方缓冲区并让算法返回。

# 实现概览

让浮点打印正确但很慢并不难（Russ Cox 已经[演示](https://research.swtch.com/ftoa)过），
让它错误但很快也不难（朴素除法和取模即可）。真正困难的是同时做到正确且高效。

广泛认为正确的算法大致有两类。

- "Dragon" algorithm family 最早由 Guy L. Steele Jr. 和 Jon L. White 描述。它们依赖
  固定大小大整数来保证正确性。后来 Robert G. Burger 和 R. Kent Dybvig 在后续论文中
  描述了一个小改进；David Gay 的 `dtoa.c` 是该策略的知名实现。

- "Grisu" algorithm family 最早由 Florian Loitsch 描述。它们使用非常便宜的纯整数过程
  找到接近正确的表示，并至少保证结果是 shortest。其变体 Grisu3 会主动检测结果表示
  是否不正确。

这里同时实现两类算法，并按 Rust 的需求做必要调整。尤其是公开文献通常不会细讲实际实现
困难，例如如何避免算术溢出。`strategy::dragon` 和 `strategy::grisu` 中各自记录了所需
论证和许多证明。（即便如此仍然很难读，提前说明。）

两个实现都暴露两个公共函数：

- `format_shortest(decoded, buf)`：始终需要至少 `MAX_SIG_DIGITS` 位数字的缓冲区，实现
  shortest mode。

- `format_exact(decoded, buf, limit)`：可接受小到 1 位数字的缓冲区，实现 exact 和 fixed
  mode。

它们会尝试用数字填充 `u8` 缓冲区，并返回已写入数字数量和 exponent `k`。对所有有限
`f32` 和 `f64` 输入它们都是全函数（必要时 Grisu 会在内部退回 Dragon）。

渲染出的数字再由四个函数格式化成实际字符串形式：

- `to_shortest_str` 打印 shortest 表示，并可补零到**至少**给定小数位数。

- `to_shortest_exp_str` 打印 shortest 表示；当 exponent 落在指定范围内时可补零，
  否则可打印为 `1.23e45` 这样的指数形式。

- `to_exact_exp_str` 以指数形式打印给定位数的 exact 表示。

- `to_exact_fixed_str` 以**恰好**给定小数位数打印 fixed 表示。

这些函数都会返回预分配 `Part` 数组中的一个切片；每个 `Part` 对应字符串的一部分：
固定字符串、已渲染数字的一段、若干零，或一个小型(`u16`)数字。调用方需要提供足够大的
缓冲区和 `Part` 数组，并自行把返回的 `Part` 组装成最终字符串。

所有算法和格式化函数都在 `coretests::num::flt2dec` 模块中有较完整测试，该模块也展示了
各函数的独立用法。

*/

// 虽然这里有大量文档，但原则上仍是私有实现；公开只是为了测试。不要把它暴露成稳定 API。
#![doc(hidden)]
#![unstable(
    feature = "flt2dec",
    reason = "internal routines only exposed for testing",
    issue = "none"
)]

pub use self::decoder::{DecodableFloat, Decoded, FullDecoded, decode};
use super::fmt::{Formatted, Part};
use crate::mem::MaybeUninit;

pub mod decoder;
pub mod estimator;

/// 数字生成算法。
pub mod strategy {
    pub mod dragon;
    pub mod grisu;
}

/// shortest mode 所需的最小缓冲区大小。
///
/// 这个值的推导并不完全直观：它等于 shortest 结果中最大有效十进制数字数量再加一。
/// 精确公式是 `ceil(mantissa 位数 * log_10 2 + 1)`。
pub const MAX_SIG_DIGITS: usize = 17;

/// 当 `d` 包含十进制数字时，增加最后一位并传播进位。
///
/// 如果进位导致长度变化，则返回新的下一位数字。
#[doc(hidden)]
pub fn round_up(d: &mut [u8]) -> Option<u8> {
    match d.iter().rposition(|&c| c != b'9') {
        Some(i) => {
            // `d[i + 1..]` 全是 9。
            d[i] += 1;
            d[i + 1..].fill(b'0');
            None
        }
        None if d.is_empty() => {
            // 空缓冲区向上舍入会得到 1；这有些奇怪但合理。
            Some(b'1')
        }
        None => {
            // 999..999 会舍入成 1000..000，并增加 exponent。
            d[0] = b'1';
            d[1..].fill(b'0');
            Some(b'0')
        }
    }
}

/// 把给定十进制数字 `0.<...buf...> * 10^exp` 格式化为十进制形式，并至少包含给定数量的
/// 小数位。
///
/// 结果写入调用方提供的 parts 数组，并返回已写入的 `Part` 切片。
///
/// `frac_digits` 可以小于 `buf` 中实际小数位数；这种情况下它会被忽略，所有已有数字都会
/// 被打印。它只用于在已渲染数字之后追加额外零。因此 `frac_digits == 0` 表示只打印给定
/// 数字，不再补任何内容。
fn digits_to_dec_str<'a>(
    buf: &'a [u8],
    exp: i16,
    frac_digits: usize,
    parts: &'a mut [MaybeUninit<Part<'a>>],
) -> &'a [Part<'a>] {
    assert!(!buf.is_empty());
    assert!(buf[0] > b'0');
    assert!(parts.len() >= 4);

    // 如果存在最后一位位置限制，则把 `buf` 视为左侧带有虚拟零。虚拟零数量 `nzeroes`
    // 等于 `max(0, exp + frac_digits - buf.len())`，从而最后一位的位置
    // `exp - buf.len() - nzeroes` 不大于 `-frac_digits`：
    //
    //                       |<-virtual->|
    //       |<---- buf ---->|  zeroes   |     exp
    //    0. 1 2 3 4 5 6 7 8 9 _ _ _ _ _ _ x 10
    //    |                  |           |
    // 10^exp    10^(exp-buf.len())   10^(exp-buf.len()-nzeroes)
    //
    // 为避免溢出，`nzeroes` 会在每种情况中单独计算。

    if exp <= 0 {
        // 小数点位于已渲染数字之前：[0.][000...000][1234][____]。
        let minus_exp = -(exp as i32) as usize;
        parts[0] = MaybeUninit::new(Part::Copy(b"0."));
        parts[1] = MaybeUninit::new(Part::Zero(minus_exp));
        parts[2] = MaybeUninit::new(Part::Copy(buf));
        if frac_digits > buf.len() && frac_digits - buf.len() > minus_exp {
            parts[3] = MaybeUninit::new(Part::Zero((frac_digits - buf.len()) - minus_exp));
            // SAFETY: 刚刚初始化了 `..4` 中的元素。
            unsafe { parts[..4].assume_init_ref() }
        } else {
            // SAFETY: 刚刚初始化了 `..3` 中的元素。
            unsafe { parts[..3].assume_init_ref() }
        }
    } else {
        let exp = exp as usize;
        if exp < buf.len() {
            // 小数点位于已渲染数字内部：[12][.][34][____]。
            parts[0] = MaybeUninit::new(Part::Copy(&buf[..exp]));
            parts[1] = MaybeUninit::new(Part::Copy(b"."));
            parts[2] = MaybeUninit::new(Part::Copy(&buf[exp..]));
            if frac_digits > buf.len() - exp {
                parts[3] = MaybeUninit::new(Part::Zero(frac_digits - (buf.len() - exp)));
                // SAFETY: 刚刚初始化了 `..4` 中的元素。
                unsafe { parts[..4].assume_init_ref() }
            } else {
                // SAFETY: 刚刚初始化了 `..3` 中的元素。
                unsafe { parts[..3].assume_init_ref() }
            }
        } else {
            // 小数点位于已渲染数字之后：[1234][____0000] 或 [1234][__][.][__]。
            parts[0] = MaybeUninit::new(Part::Copy(buf));
            parts[1] = MaybeUninit::new(Part::Zero(exp - buf.len()));
            if frac_digits > 0 {
                parts[2] = MaybeUninit::new(Part::Copy(b"."));
                parts[3] = MaybeUninit::new(Part::Zero(frac_digits));
                // SAFETY: 刚刚初始化了 `..4` 中的元素。
                unsafe { parts[..4].assume_init_ref() }
            } else {
                // SAFETY: 刚刚初始化了 `..2` 中的元素。
                unsafe { parts[..2].assume_init_ref() }
            }
        }
    }
}

/// 把给定十进制数字 `0.<...buf...> * 10^exp` 格式化为指数形式，并至少包含给定数量的
/// 有效数字。
///
/// 当 `upper` 为 `true` 时，exponent 前缀使用 `E`；否则使用 `e`。结果写入调用方提供的
/// parts 数组，并返回已写入的 `Part` 切片。
///
/// `min_digits` 可以小于 `buf` 中实际有效数字数量；这种情况下它会被忽略，所有已有数字
/// 都会被打印。它只用于在已渲染数字之后追加额外零。因此 `min_digits == 0` 表示只打印
/// 给定数字，不再补任何内容。
fn digits_to_exp_str<'a>(
    buf: &'a [u8],
    exp: i16,
    min_ndigits: usize,
    upper: bool,
    parts: &'a mut [MaybeUninit<Part<'a>>],
) -> &'a [Part<'a>] {
    assert!(!buf.is_empty());
    assert!(buf[0] > b'0');
    assert!(parts.len() >= 6);

    let mut n = 0;

    parts[n] = MaybeUninit::new(Part::Copy(&buf[..1]));
    n += 1;

    if buf.len() > 1 || min_ndigits > 1 {
        parts[n] = MaybeUninit::new(Part::Copy(b"."));
        parts[n + 1] = MaybeUninit::new(Part::Copy(&buf[1..]));
        n += 2;
        if min_ndigits > buf.len() {
            parts[n] = MaybeUninit::new(Part::Zero(min_ndigits - buf.len()));
            n += 1;
        }
    }

    // 0.1234 x 10^exp = 1.234 x 10^(exp-1)。
    let exp = exp as i32 - 1; // 避免 `exp == i16::MIN` 时下溢。
    if exp < 0 {
        parts[n] = MaybeUninit::new(Part::Copy(if upper { b"E-" } else { b"e-" }));
        parts[n + 1] = MaybeUninit::new(Part::Num(-exp as u16));
    } else {
        parts[n] = MaybeUninit::new(Part::Copy(if upper { b"E" } else { b"e" }));
        parts[n + 1] = MaybeUninit::new(Part::Num(exp as u16));
    }
    // SAFETY: 刚刚初始化了 `..n + 2` 中的元素。
    unsafe { parts[..n + 2].assume_init_ref() }
}

/// 符号格式化选项。
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Sign {
    /// 对任意负值打印 `-`。
    Minus, // -inf -1 -0  0  1  inf nan
    /// 对任意负值打印 `-`，否则打印 `+`。
    MinusPlus, // -inf -1 -0 +0 +1 +inf nan
}

/// 返回待格式化符号对应的静态字节字符串。
///
/// 结果只可能是 `""`、`"+"` 或 `"-"`。
fn determine_sign(sign: Sign, decoded: &FullDecoded, negative: bool) -> &'static str {
    match (*decoded, sign) {
        (FullDecoded::Nan, _) => "",
        (_, Sign::Minus) => {
            if negative {
                "-"
            } else {
                ""
            }
        }
        (_, Sign::MinusPlus) => {
            if negative {
                "-"
            } else {
                "+"
            }
        }
    }
}

/// 把给定浮点数格式化为十进制形式，并至少包含给定数量的小数位。
///
/// 结果写入调用方提供的 parts 数组，同时使用给定字节缓冲区作为 scratch。`upper` 目前未用，
/// 但保留给未来决定非有限值（`inf` 和 `nan`）大小写。渲染的第一部分始终是符号字符串，
/// 如果不需要符号则为空字符串。
///
/// `format_shortest` 应是底层数字生成函数。它需要返回自己初始化过的缓冲区部分。
/// 通常这里会使用 `strategy::grisu::format_shortest`。
///
/// `frac_digits` 可以小于 `v` 的实际小数位数；这种情况下它会被忽略，所有已有数字都会
/// 被打印。它只用于在已渲染数字之后追加额外零。因此 `frac_digits == 0` 表示只打印给定
/// 数字，不再补任何内容。
///
/// 字节缓冲区长度至少应为 `MAX_SIG_DIGITS`。parts 至少要有 4 个元素，因为最坏情况类似
/// `frac_digits = 10` 时的 `[+][0.][0000][2][0000]`。
pub fn to_shortest_str<'a, T, F>(
    mut format_shortest: F,
    v: T,
    sign: Sign,
    frac_digits: usize,
    buf: &'a mut [MaybeUninit<u8>],
    parts: &'a mut [MaybeUninit<Part<'a>>],
) -> Formatted<'a>
where
    T: DecodableFloat,
    F: FnMut(&Decoded, &'a mut [MaybeUninit<u8>]) -> (&'a [u8], i16),
{
    assert!(parts.len() >= 4);
    assert!(buf.len() >= MAX_SIG_DIGITS);

    let (negative, full_decoded) = decode(v);
    let sign = determine_sign(sign, &full_decoded, negative);
    match full_decoded {
        FullDecoded::Nan => {
            parts[0] = MaybeUninit::new(Part::Copy(b"NaN"));
            // SAFETY: 刚刚初始化了 `..1` 中的元素。
            Formatted { sign, parts: unsafe { parts[..1].assume_init_ref() } }
        }
        FullDecoded::Infinite => {
            parts[0] = MaybeUninit::new(Part::Copy(b"inf"));
            // SAFETY: 刚刚初始化了 `..1` 中的元素。
            Formatted { sign, parts: unsafe { parts[..1].assume_init_ref() } }
        }
        FullDecoded::Zero => {
            if frac_digits > 0 {
                // [0.][0000]
                parts[0] = MaybeUninit::new(Part::Copy(b"0."));
                parts[1] = MaybeUninit::new(Part::Zero(frac_digits));
                Formatted {
                    sign,
                    // SAFETY: 刚刚初始化了 `..2` 中的元素。
                    parts: unsafe { parts[..2].assume_init_ref() },
                }
            } else {
                parts[0] = MaybeUninit::new(Part::Copy(b"0"));
                Formatted {
                    sign,
                    // SAFETY: 刚刚初始化了 `..1` 中的元素。
                    parts: unsafe { parts[..1].assume_init_ref() },
                }
            }
        }
        FullDecoded::Finite(ref decoded) => {
            let (buf, exp) = format_shortest(decoded, buf);
            Formatted { sign, parts: digits_to_dec_str(buf, exp, frac_digits, parts) }
        }
    }
}

/// 根据结果 exponent，把给定浮点数格式化为十进制形式或指数形式。
///
/// 结果写入调用方提供的 parts 数组，同时使用给定字节缓冲区作为 scratch。`upper` 用来决定
/// 非有限值（`inf` 和 `nan`）大小写，或 exponent 前缀使用 `e` 还是 `E`。渲染的第一部分
/// 始终是符号字符串；如果不需要符号则为空字符串。
///
/// `format_shortest` 应是底层数字生成函数。它需要返回自己初始化过的缓冲区部分。
/// 通常这里会使用 `strategy::grisu::format_shortest`。
///
/// `dec_bounds` 是 `(lo, hi)` 元组：只有当 `10^lo <= V < 10^hi` 时才使用十进制形式。
/// 注意这里的 `V` 是**表观**十进制值，而不是实际 `v`。因此指数形式中打印出的 exponent
/// 不会落在该范围内，避免歧义。
///
/// 字节缓冲区长度至少应为 `MAX_SIG_DIGITS`。parts 至少要有 6 个元素，因为最坏情况类似
/// `[+][1][.][2345][e][-][6]`。
pub fn to_shortest_exp_str<'a, T, F>(
    mut format_shortest: F,
    v: T,
    sign: Sign,
    dec_bounds: (i16, i16),
    upper: bool,
    buf: &'a mut [MaybeUninit<u8>],
    parts: &'a mut [MaybeUninit<Part<'a>>],
) -> Formatted<'a>
where
    T: DecodableFloat,
    F: FnMut(&Decoded, &'a mut [MaybeUninit<u8>]) -> (&'a [u8], i16),
{
    assert!(parts.len() >= 6);
    assert!(buf.len() >= MAX_SIG_DIGITS);
    assert!(dec_bounds.0 <= dec_bounds.1);

    let (negative, full_decoded) = decode(v);
    let sign = determine_sign(sign, &full_decoded, negative);
    match full_decoded {
        FullDecoded::Nan => {
            parts[0] = MaybeUninit::new(Part::Copy(b"NaN"));
            // SAFETY: 刚刚初始化了 `..1` 中的元素。
            Formatted { sign, parts: unsafe { parts[..1].assume_init_ref() } }
        }
        FullDecoded::Infinite => {
            parts[0] = MaybeUninit::new(Part::Copy(b"inf"));
            // SAFETY: 刚刚初始化了 `..1` 中的元素。
            Formatted { sign, parts: unsafe { parts[..1].assume_init_ref() } }
        }
        FullDecoded::Zero => {
            parts[0] = if dec_bounds.0 <= 0 && 0 < dec_bounds.1 {
                MaybeUninit::new(Part::Copy(b"0"))
            } else {
                MaybeUninit::new(Part::Copy(if upper { b"0E0" } else { b"0e0" }))
            };
            // SAFETY: 刚刚初始化了 `..1` 中的元素。
            Formatted { sign, parts: unsafe { parts[..1].assume_init_ref() } }
        }
        FullDecoded::Finite(ref decoded) => {
            let (buf, exp) = format_shortest(decoded, buf);
            let vis_exp = exp as i32 - 1;
            let parts = if dec_bounds.0 as i32 <= vis_exp && vis_exp < dec_bounds.1 as i32 {
                digits_to_dec_str(buf, exp, 0, parts)
            } else {
                digits_to_exp_str(buf, exp, 0, upper, parts)
            };
            Formatted { sign, parts }
        }
    }
}

/// 根据解码后的 exponent 返回最大缓冲区大小的粗略近似（上界）。
///
/// 精确上界为：
///
/// - 当 `exp < 0` 时，最大长度是 `ceil(log_10 (5^-exp * (2^64 - 1)))`。
/// - 当 `exp >= 0` 时，最大长度是 `ceil(log_10 (2^exp * (2^64 - 1)))`。
///
/// `ceil(log_10 (x^exp * (2^64 - 1)))` 小于
/// `ceil(log_10 (2^64 - 1)) + ceil(exp * log_10 x)`，后者又小于
/// `20 + (1 + exp * log_10 x)`。这里利用 `log_10 2 < 5/16` 和
/// `log_10 5 < 12/16`，对我们的上界估计已经足够。
///
/// 为什么需要该上界？`format_exact` 函数会填满整个缓冲区，除非受到最后一位限制约束；
/// 但调用方请求的数字位数可能极大（例如 30,000 位）。其中绝大多数缓冲区都会被零填充，
/// 因此不希望预先分配完整大小。由此可知，对任意参数，`f64` 使用 826 字节缓冲区就足够；
/// 作为对比，实际最坏情况（`exp = -1074`）需要 770 字节。
fn estimate_max_buf_len(exp: i16) -> usize {
    21 + ((if exp < 0 { -12 } else { 5 } * exp as i32) as usize >> 4)
}

/// 把给定浮点数格式化为指数形式，并恰好包含给定数量的有效数字。
///
/// 结果写入调用方提供的 parts 数组，同时使用给定字节缓冲区作为 scratch。`upper` 用来决定
/// exponent 前缀使用 `e` 还是 `E`。渲染的第一部分始终是符号字符串；如果不需要符号则
/// 为空字符串。
///
/// `format_exact` 应是底层数字生成函数。它需要返回自己初始化过的缓冲区部分。
/// 通常这里会使用 `strategy::grisu::format_exact`。
///
/// 字节缓冲区长度至少应为 `ndigits`，除非 `ndigits` 大到算法无论如何只会写入固定数量数字。
/// 对 `f64`，临界点约为 800，因此 1000 字节应当足够。parts 至少要有 6 个元素，因为
/// 最坏情况类似 `[+][1][.][2345][e][-][6]`。
pub fn to_exact_exp_str<'a, T, F>(
    mut format_exact: F,
    v: T,
    sign: Sign,
    ndigits: usize,
    upper: bool,
    buf: &'a mut [MaybeUninit<u8>],
    parts: &'a mut [MaybeUninit<Part<'a>>],
) -> Formatted<'a>
where
    T: DecodableFloat,
    F: FnMut(&Decoded, &'a mut [MaybeUninit<u8>], i16) -> (&'a [u8], i16),
{
    assert!(parts.len() >= 6);
    assert!(ndigits > 0);

    let (negative, full_decoded) = decode(v);
    let sign = determine_sign(sign, &full_decoded, negative);
    match full_decoded {
        FullDecoded::Nan => {
            parts[0] = MaybeUninit::new(Part::Copy(b"NaN"));
            // SAFETY: 刚刚初始化了 `..1` 中的元素。
            Formatted { sign, parts: unsafe { parts[..1].assume_init_ref() } }
        }
        FullDecoded::Infinite => {
            parts[0] = MaybeUninit::new(Part::Copy(b"inf"));
            // SAFETY: 刚刚初始化了 `..1` 中的元素。
            Formatted { sign, parts: unsafe { parts[..1].assume_init_ref() } }
        }
        FullDecoded::Zero => {
            if ndigits > 1 {
                // [0.][0000][e0]
                parts[0] = MaybeUninit::new(Part::Copy(b"0."));
                parts[1] = MaybeUninit::new(Part::Zero(ndigits - 1));
                parts[2] = MaybeUninit::new(Part::Copy(if upper { b"E0" } else { b"e0" }));
                Formatted {
                    sign,
                    // SAFETY: 刚刚初始化了 `..3` 中的元素。
                    parts: unsafe { parts[..3].assume_init_ref() },
                }
            } else {
                parts[0] = MaybeUninit::new(Part::Copy(if upper { b"0E0" } else { b"0e0" }));
                Formatted {
                    sign,
                    // SAFETY: 刚刚初始化了 `..1` 中的元素。
                    parts: unsafe { parts[..1].assume_init_ref() },
                }
            }
        }
        FullDecoded::Finite(ref decoded) => {
            let maxlen = estimate_max_buf_len(decoded.exp);
            assert!(buf.len() >= ndigits || buf.len() >= maxlen);

            let trunc = if ndigits < maxlen { ndigits } else { maxlen };
            let (buf, exp) = format_exact(decoded, &mut buf[..trunc], i16::MIN);
            Formatted { sign, parts: digits_to_exp_str(buf, exp, ndigits, upper, parts) }
        }
    }
}

/// 把给定浮点数格式化为十进制形式，并恰好包含给定数量的小数位。
///
/// 结果写入调用方提供的 parts 数组，同时使用给定字节缓冲区作为 scratch。`upper` 目前未用，
/// 但保留给未来决定非有限值（`inf` 和 `nan`）大小写。渲染的第一部分始终是符号字符串；
/// 如果不需要符号则为空字符串。
///
/// `format_exact` 应是底层数字生成函数。它需要返回自己初始化过的缓冲区部分。
/// 通常这里会使用 `strategy::grisu::format_exact`。
///
/// 字节缓冲区应足以容纳输出，除非 `frac_digits` 大到算法无论如何只会写入固定数量数字。
/// 对 `f64`，临界点约为 800，因此 1000 字节应当足够。parts 至少要有 4 个元素，因为
/// 最坏情况类似 `frac_digits = 10` 时的 `[+][0.][0000][2][0000]`。
pub fn to_exact_fixed_str<'a, T, F>(
    mut format_exact: F,
    v: T,
    sign: Sign,
    frac_digits: usize,
    buf: &'a mut [MaybeUninit<u8>],
    parts: &'a mut [MaybeUninit<Part<'a>>],
) -> Formatted<'a>
where
    T: DecodableFloat,
    F: FnMut(&Decoded, &'a mut [MaybeUninit<u8>], i16) -> (&'a [u8], i16),
{
    assert!(parts.len() >= 4);

    let (negative, full_decoded) = decode(v);
    let sign = determine_sign(sign, &full_decoded, negative);
    match full_decoded {
        FullDecoded::Nan => {
            parts[0] = MaybeUninit::new(Part::Copy(b"NaN"));
            // SAFETY: 刚刚初始化了 `..1` 中的元素。
            Formatted { sign, parts: unsafe { parts[..1].assume_init_ref() } }
        }
        FullDecoded::Infinite => {
            parts[0] = MaybeUninit::new(Part::Copy(b"inf"));
            // SAFETY: 刚刚初始化了 `..1` 中的元素。
            Formatted { sign, parts: unsafe { parts[..1].assume_init_ref() } }
        }
        FullDecoded::Zero => {
            if frac_digits > 0 {
                // [0.][0000]
                parts[0] = MaybeUninit::new(Part::Copy(b"0."));
                parts[1] = MaybeUninit::new(Part::Zero(frac_digits));
                Formatted {
                    sign,
                    // SAFETY: 刚刚初始化了 `..2` 中的元素。
                    parts: unsafe { parts[..2].assume_init_ref() },
                }
            } else {
                parts[0] = MaybeUninit::new(Part::Copy(b"0"));
                Formatted {
                    sign,
                    // SAFETY: 刚刚初始化了 `..1` 中的元素。
                    parts: unsafe { parts[..1].assume_init_ref() },
                }
            }
        }
        FullDecoded::Finite(ref decoded) => {
            let maxlen = estimate_max_buf_len(decoded.exp);
            assert!(buf.len() >= maxlen);

            // `frac_digits` 的确可能大得离谱。此时 `format_exact` 会早得多地结束数字渲染，
            // 因为我们严格受 `maxlen` 限制。
            let limit = if frac_digits < 0x8000 { -(frac_digits as i16) } else { i16::MIN };
            let (buf, exp) = format_exact(decoded, &mut buf[..maxlen], limit);
            if exp <= limit {
                // 无法满足最后一位限制，因此无论 `exp` 是什么都应渲染得像零。这不包括只有
                // 经过最终向上舍入后才满足限制的情况；那是 `exp = limit + 1` 的常规路径。
                debug_assert_eq!(buf.len(), 0);
                if frac_digits > 0 {
                    // [0.][0000]
                    parts[0] = MaybeUninit::new(Part::Copy(b"0."));
                    parts[1] = MaybeUninit::new(Part::Zero(frac_digits));
                    Formatted {
                        sign,
                    // SAFETY: 刚刚初始化了 `..2` 中的元素。
                        parts: unsafe { parts[..2].assume_init_ref() },
                    }
                } else {
                    parts[0] = MaybeUninit::new(Part::Copy(b"0"));
                    Formatted {
                        sign,
                        // SAFETY: 刚刚初始化了 `..1` 中的元素。
                        parts: unsafe { parts[..1].assume_init_ref() },
                    }
                }
            } else {
                Formatted { sign, parts: digits_to_dec_str(buf, exp, frac_digits, parts) }
            }
        }
    }
}
