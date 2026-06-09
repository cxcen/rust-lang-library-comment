use crate::fmt::{Debug, Display, Formatter, LowerExp, Result, UpperExp};
use crate::mem::MaybeUninit;
use crate::num::{flt2dec, fmt as numfmt};

#[doc(hidden)]
trait GeneralFormat: PartialOrd {
    /// 在“显示前不会再被进一步舍入”的前提下,根据数值大小判断是否应使用指数格式。
    ///
    /// 这对应 `Debug` 的一般格式选择:很小或很大的浮点数用指数形式更清晰,
    /// 其余值则优先使用普通十进制形式。
    fn already_rounded_value_should_use_exponential(&self) -> bool;
}

macro_rules! impl_general_format {
    ($($t:ident)*) => {
        $(impl GeneralFormat for $t {
            fn already_rounded_value_should_use_exponential(&self) -> bool {
                let abs = $t::abs(*self);
                (abs != 0.0 && abs < 1e-4) || abs >= 1e+16
            }
        })*
    }
}

#[cfg(target_has_reliable_f16)]
impl_general_format! { f16 }
impl_general_format! { f32 f64 }

// 不内联此函数,这样调用方只有确实走到精确十进制格式化路径时,
// 才会承担该函数所需的栈空间。
#[inline(never)]
fn float_to_decimal_common_exact<T>(
    fmt: &mut Formatter<'_>,
    num: &T,
    sign: flt2dec::Sign,
    precision: u16,
) -> Result
where
    T: flt2dec::DecodableFloat,
{
    let mut buf: [MaybeUninit<u8>; 1024] = [MaybeUninit::uninit(); 1024]; // 对 f32 和 f64 足够大
    let mut parts: [MaybeUninit<numfmt::Part<'_>>; 4] = [MaybeUninit::uninit(); 4];
    let formatted = flt2dec::to_exact_fixed_str(
        flt2dec::strategy::grisu::format_exact,
        *num,
        sign,
        precision.into(),
        &mut buf,
        &mut parts,
    );
    // SAFETY: `to_exact_fixed_str` 和 `format_exact` 只会产生 ASCII 字符。
    unsafe { fmt.pad_formatted_parts(&formatted) }
}

// 不内联此函数,避免同时调用本函数和上面函数的调用方在某些情况下
// 承担两条格式化路径合并后的栈空间。
#[inline(never)]
fn float_to_decimal_common_shortest<T>(
    fmt: &mut Formatter<'_>,
    num: &T,
    sign: flt2dec::Sign,
    precision: u16,
) -> Result
where
    T: flt2dec::DecodableFloat,
{
    // 对 f32 和 f64 足够大
    let mut buf: [MaybeUninit<u8>; flt2dec::MAX_SIG_DIGITS] =
        [MaybeUninit::uninit(); flt2dec::MAX_SIG_DIGITS];
    let mut parts: [MaybeUninit<numfmt::Part<'_>>; 4] = [MaybeUninit::uninit(); 4];
    let formatted = flt2dec::to_shortest_str(
        flt2dec::strategy::grisu::format_shortest,
        *num,
        sign,
        precision.into(),
        &mut buf,
        &mut parts,
    );
    // SAFETY: `to_shortest_str` 和 `format_shortest` 只会产生 ASCII 字符。
    unsafe { fmt.pad_formatted_parts(&formatted) }
}

fn float_to_decimal_display<T>(fmt: &mut Formatter<'_>, num: &T) -> Result
where
    T: flt2dec::DecodableFloat,
{
    let force_sign = fmt.sign_plus();
    let sign = match force_sign {
        false => flt2dec::Sign::Minus,
        true => flt2dec::Sign::MinusPlus,
    };

    if let Some(precision) = fmt.options.get_precision() {
        float_to_decimal_common_exact(fmt, num, sign, precision)
    } else {
        let min_precision = 0;
        float_to_decimal_common_shortest(fmt, num, sign, min_precision)
    }
}

// 不内联此函数,这样调用方只有确实走到精确指数格式化路径时,
// 才会承担该函数所需的栈空间。
#[inline(never)]
fn float_to_exponential_common_exact<T>(
    fmt: &mut Formatter<'_>,
    num: &T,
    sign: flt2dec::Sign,
    precision: u16,
    upper: bool,
) -> Result
where
    T: flt2dec::DecodableFloat,
{
    let mut buf: [MaybeUninit<u8>; 1024] = [MaybeUninit::uninit(); 1024]; // 对 f32 和 f64 足够大
    let mut parts: [MaybeUninit<numfmt::Part<'_>>; 6] = [MaybeUninit::uninit(); 6];
    let formatted = flt2dec::to_exact_exp_str(
        flt2dec::strategy::grisu::format_exact,
        *num,
        sign,
        precision.into(),
        upper,
        &mut buf,
        &mut parts,
    );
    // SAFETY: `to_exact_exp_str` 和 `format_exact` 只会产生 ASCII 字符。
    unsafe { fmt.pad_formatted_parts(&formatted) }
}

// 不内联此函数,避免同时调用本函数和上面函数的调用方在某些情况下
// 承担两条格式化路径合并后的栈空间。
#[inline(never)]
fn float_to_exponential_common_shortest<T>(
    fmt: &mut Formatter<'_>,
    num: &T,
    sign: flt2dec::Sign,
    upper: bool,
) -> Result
where
    T: flt2dec::DecodableFloat,
{
    // 对 f32 和 f64 足够大
    let mut buf: [MaybeUninit<u8>; flt2dec::MAX_SIG_DIGITS] =
        [MaybeUninit::uninit(); flt2dec::MAX_SIG_DIGITS];
    let mut parts: [MaybeUninit<numfmt::Part<'_>>; 6] = [MaybeUninit::uninit(); 6];
    let formatted = flt2dec::to_shortest_exp_str(
        flt2dec::strategy::grisu::format_shortest,
        *num,
        sign,
        (0, 0),
        upper,
        &mut buf,
        &mut parts,
    );
    // SAFETY: `to_shortest_exp_str` 和 `format_shortest` 只会产生 ASCII 字符。
    unsafe { fmt.pad_formatted_parts(&formatted) }
}

// 浮点数 `LowerExp` 与 `UpperExp` 共用的格式化逻辑。
fn float_to_exponential_common<T>(fmt: &mut Formatter<'_>, num: &T, upper: bool) -> Result
where
    T: flt2dec::DecodableFloat,
{
    let force_sign = fmt.sign_plus();
    let sign = match force_sign {
        false => flt2dec::Sign::Minus,
        true => flt2dec::Sign::MinusPlus,
    };

    if let Some(precision) = fmt.options.get_precision() {
        // 1 个整数位 + `precision` 个小数位 = 总计 `precision + 1` 个有效数字。
        float_to_exponential_common_exact(fmt, num, sign, precision + 1, upper)
    } else {
        float_to_exponential_common_shortest(fmt, num, sign, upper)
    }
}

fn float_to_general_debug<T>(fmt: &mut Formatter<'_>, num: &T) -> Result
where
    T: flt2dec::DecodableFloat + GeneralFormat,
{
    let force_sign = fmt.sign_plus();
    let sign = match force_sign {
        false => flt2dec::Sign::Minus,
        true => flt2dec::Sign::MinusPlus,
    };

    if let Some(precision) = fmt.options.get_precision() {
        // `{:.PREC?}` 的这一行为早于 `{:?}` 的指数格式化支持。
        float_to_decimal_common_exact(fmt, num, sign, precision)
    } else {
        // 没有精度参数时不会发生舍入,因此可以基于当前值直接选择格式。
        if num.already_rounded_value_should_use_exponential() {
            let upper = false;
            float_to_exponential_common_shortest(fmt, num, sign, upper)
        } else {
            let min_precision = 1;
            float_to_decimal_common_shortest(fmt, num, sign, min_precision)
        }
    }
}

macro_rules! floating {
    ($($ty:ident)*) => {
        $(
            #[stable(feature = "rust1", since = "1.0.0")]
            impl Debug for $ty {
                fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
                    float_to_general_debug(fmt, self)
                }
            }

            #[stable(feature = "rust1", since = "1.0.0")]
            impl Display for $ty {
                fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
                    float_to_decimal_display(fmt, self)
                }
            }

            #[stable(feature = "rust1", since = "1.0.0")]
            impl LowerExp for $ty {
                fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
                    float_to_exponential_common(fmt, self, false)
                }
            }

            #[stable(feature = "rust1", since = "1.0.0")]
            impl UpperExp for $ty {
                fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
                    float_to_exponential_common(fmt, self, true)
                }
            }
        )*
    };
}

floating! { f32 f64 }

#[cfg(target_has_reliable_f16)]
floating! { f16 }

// FIXME(f16_f128): 当后端+目标平台尚不能良好支持 f16 时使用回退实现,
// 以避免 ICE。

#[cfg(not(target_has_reliable_f16))]
#[stable(feature = "rust1", since = "1.0.0")]
impl Debug for f16 {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{:#06x}", self.to_bits())
    }
}

#[cfg(not(target_has_reliable_f16))]
#[stable(feature = "rust1", since = "1.0.0")]
impl Display for f16 {
    #[inline]
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
        Debug::fmt(self, fmt)
    }
}

#[cfg(not(target_has_reliable_f16))]
#[stable(feature = "rust1", since = "1.0.0")]
impl LowerExp for f16 {
    #[inline]
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
        Debug::fmt(self, fmt)
    }
}

#[cfg(not(target_has_reliable_f16))]
#[stable(feature = "rust1", since = "1.0.0")]
impl UpperExp for f16 {
    #[inline]
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
        Debug::fmt(self, fmt)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Debug for f128 {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{:#034x}", self.to_bits())
    }
}
