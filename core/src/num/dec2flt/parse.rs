//! 浮点数字面量的解析函数。
//!
//! 本模块只做语法拆解和十进制 `(mantissa, exponent)` 归约，不负责最终 IEEE 754 舍入。
//! 空输入和不符合文法的输入会在上层分别映射成 `ParseFloatError` 的分类。

use crate::num::dec2flt::common::{ByteSlice, is_8digits};
use crate::num::dec2flt::decimal::Decimal;
use crate::num::dec2flt::float::RawFloat;

const MIN_19DIGIT_INT: u64 = 100_0000_0000_0000_0000;

/// 解析按小端序加载到整数中的 8 个十进制数字。
///
/// 该函数利用每个数字字节都位于 `[0x30, 0x39]` 的事实，只用 3 次乘法就能把 8 个
/// ASCII 数字合成为整数，比逐位做 8 次乘法更快。
///
/// 算法基于 "Fast numeric string to int"：
/// <https://johnnylee-sde.github.io/Fast-numeric-string-to-int/>。
fn parse_8digits(mut v: u64) -> u64 {
    const MASK: u64 = 0x0000_00FF_0000_00FF;
    const MUL1: u64 = 0x000F_4240_0000_0064;
    const MUL2: u64 = 0x0000_2710_0000_0001;
    v -= 0x3030_3030_3030_3030;
    v = (v * 10) + (v >> 8); // 不会溢出，结果可放入 63 位。
    let v1 = (v & MASK).wrapping_mul(MUL1);
    let v2 = ((v >> 16) & MASK).wrapping_mul(MUL2);
    ((v1.wrapping_add(v2) >> 32) as u32) as u64
}

/// 持续解析数字，直到遇到非数字字符。
fn try_parse_digits(mut s: &[u8], mut x: u64) -> (&[u8], u64) {
    // 这里允许发生 wrapping，后续路径会根据位数和指数判断是否需要慢速精确处理。

    while s.len() >= 8 {
        let num = s.read_u64();
        if is_8digits(num) {
            x = x.wrapping_mul(1_0000_0000).wrapping_add(parse_8digits(num));
            s = &s[8..];
        } else {
            break;
        }
    }

    s = s.parse_digits(|digit| {
        x = x.wrapping_mul(10).wrapping_add(digit as _);
    });

    (s, x)
}

/// 最多解析 19 位数字，这是 64 位整数能容纳的最大十进制位数。
fn try_parse_19digits(s_ref: &mut &[u8], x: &mut u64) {
    let mut s = *s_ref;

    while *x < MIN_19DIGIT_INT {
        if let Some((c, s_next)) = s.split_first() {
            let digit = c.wrapping_sub(b'0');

            if digit < 10 {
                *x = (*x * 10) + digit as u64; // 受 19 位限制保护，这里不会溢出。
                s = s_next;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    *s_ref = s;
}

/// 解析浮点数字面量中的科学计数法指数部分。
fn parse_scientific(s_ref: &mut &[u8]) -> Option<i64> {
    let mut exponent = 0i64;
    let mut negative = false;

    let mut s = *s_ref;

    if let Some((&c, s_next)) = s.split_first() {
        negative = c == b'-';
        if c == b'-' || c == b'+' {
            s = s_next;
        }
    }

    if matches!(s.first(), Some(&x) if x.is_ascii_digit()) {
        *s_ref = s.parse_digits(|digit| {
            // 在接近溢出之前就停止增长；后续只需要知道指数足够大。
            if exponent < 0x10000 {
                exponent = 10 * exponent + digit as i64;
            }
        });
        if negative { Some(-exponent) } else { Some(exponent) }
    } else {
        *s_ref = s;
        None
    }
}

/// 解析一个普通（非 Inf/NaN）浮点数字面量前缀。
///
/// 返回值把浮点数表示成十进制有效数字和十进制指数，并记录实际消费的字节数。
fn parse_partial_number(mut s: &[u8]) -> Option<(Decimal, usize)> {
    debug_assert!(!s.is_empty());

    // 解析小数点前的初始数字。
    let mut mantissa = 0_u64;
    let start = s;
    let tmp = try_parse_digits(s, mantissa);
    s = tmp.0;
    mantissa = tmp.1;
    let mut n_digits = s.offset_from(start);

    // 处理小数点以及其后的数字。
    let mut n_after_dot = 0;
    let mut exponent = 0_i64;
    let int_end = s;

    if let Some((&b'.', s_next)) = s.split_first() {
        s = s_next;
        let before = s;
        let tmp = try_parse_digits(s, mantissa);
        s = tmp.0;
        mantissa = tmp.1;
        n_after_dot = s.offset_from(before);
        exponent = -n_after_dot as i64;
    }

    n_digits += n_after_dot;
    if n_digits == 0 {
        return None;
    }

    // 处理科学计数法格式。
    let mut exp_number = 0_i64;
    if let Some((&c, s_next)) = s.split_first() {
        if c == b'e' || c == b'E' {
            s = s_next;
            // 若返回 None，说明指数后没有数字，或整体不是有效浮点数字面量。
            exp_number = parse_scientific(&mut s)?;
            exponent += exp_number;
        }
    }

    let len = s.offset_from(start) as _;

    // 处理有效数字很多的非常规情况。
    if n_digits <= 19 {
        return Some((Decimal { exponent, mantissa, negative: false, many_digits: false }, len));
    }

    n_digits -= 19;
    let mut many_digits = false;
    let mut p = start;
    while let Some((&c, p_next)) = p.split_first() {
        if c == b'.' || c == b'0' {
            n_digits -= c.saturating_sub(b'0' - 1) as isize;
            p = p_next;
        } else {
            break;
        }
    }
    if n_digits > 0 {
        // 到这里说明存在超过 19 位有效数字，需要重新解析可放进 `u64` 的前缀。
        many_digits = true;
        mantissa = 0;
        let mut s = start;
        try_parse_19digits(&mut s, &mut mantissa);
        exponent = if mantissa >= MIN_19DIGIT_INT {
            // 需要大整数慢路径。
            int_end.offset_from(s)
        } else {
            s = &s[1..];
            let before = s;
            try_parse_19digits(&mut s, &mut mantissa);
            -s.offset_from(before)
        } as i64;
        // 加回显式指数部分。
        exponent += exp_number;
    }

    Some((Decimal { exponent, mantissa, negative: false, many_digits }, len))
}

/// 尝试解析普通（非 Inf/NaN）浮点数。
///
/// 只有整个输入都被普通浮点文法消费时才返回 `Some`；否则返回 `None`，让上层继续尝试
/// 特殊值或报告无效字面量。
pub fn parse_number(s: &[u8]) -> Option<Decimal> {
    if let Some((float, rest)) = parse_partial_number(s) {
        if rest == s.len() {
            return Some(float);
        }
    }
    None
}

/// 尝试解析特殊的非有限浮点值。
pub(crate) fn parse_inf_nan<F: RawFloat>(s: &[u8], negative: bool) -> Option<F> {
    // 有效字符串最长只有 8 个字节，因此可以把所有相关字符装入一个 `u64` 再比较。
    // 这样也会生成更紧凑、更快的代码。

    let mut register;
    let len: usize;

    // 有效特殊值字符串长度只可能是 8 或 3。
    if s.len() == 8 {
        register = s.read_u64();
        len = 8;
    } else if s.len() == 3 {
        let a = s[0] as u64;
        let b = s[1] as u64;
        let c = s[2] as u64;
        register = (c << 16) | (b << 8) | a;
        len = 3;
    } else {
        return None;
    }

    // 清掉会把 ASCII 大写字符变成小写字符的位；结果字符串等价于全大写。
    // 对其他字符会发生什么并不重要，因为后续常量比较会拒绝它们。
    register &= 0xDFDFDFDFDFDFDFDF;

    // 与相关特殊值对应的 `u64` 常量。
    const INF_3: u64 = 0x464E49; // "INF"
    const INF_8: u64 = 0x5954494E49464E49; // "INFINITY"
    const NAN: u64 = 0x4E414E; // "NAN"

    // 通过寄存器值匹配常量来解析字符串，同时匹配长度以排除
    // `"inf\0\0\0\0\0"` 这类边界情况。
    let float = match (register, len) {
        (INF_3, 3) => F::INFINITY,
        (INF_8, 8) => F::INFINITY,
        (NAN, 3) => F::NAN,
        _ => return None,
    };

    if negative { Some(-float) } else { Some(float) }
}
