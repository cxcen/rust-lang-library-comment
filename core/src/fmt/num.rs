//! 整数与浮点数的格式化实现。
//!
//! 本模块位于 `core::fmt` 的热路径上,被 `Display`、`Debug`、`write!`、
//! `format_args!`、panic 消息和日志输出共同依赖。实现目标是在不分配堆内存的前提下,
//! 把数值写入栈上缓冲区,再把已经初始化且只含 ASCII 数字的后缀交给 `Formatter`。

use crate::fmt::NumBuffer;
use crate::mem::MaybeUninit;
use crate::num::fmt as numfmt;
use crate::{fmt, str};

/// 非十进制基数的整数格式化。
macro_rules! radix_integer {
    (fmt::$Trait:ident for $Signed:ident and $Unsigned:ident, $prefix:literal, $dig_tab:literal) => {
        #[stable(feature = "rust1", since = "1.0.0")]
        impl fmt::$Trait for $Unsigned {
            /// 按指定基数格式化无符号整数。
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                // 在编译期检查宏参数,确保表驱动格式化的前提成立。
                const {
                    assert!($Unsigned::MIN == 0, "need unsigned");
                    assert!($dig_tab.is_ascii(), "need single-byte entries");
                }

                // 按升序排列的 ASCII 数字作为查找表使用。
                const DIG_TAB: &[u8] = $dig_tab;
                const BASE: $Unsigned = DIG_TAB.len() as $Unsigned;
                const MAX_DIG_N: usize = $Unsigned::MAX.ilog(BASE) as usize + 1;

                // 从缓冲区右侧向左写入 `self` 的各位数字。
                let mut buf = [MaybeUninit::<u8>::uninit(); MAX_DIG_N];
                // `offset` 记录 `buf` 前缀中尚未初始化的字节数。
                let mut offset = buf.len();

                // 从最低有效位到最高有效位逐步取出数字并写入缓冲区。
                let mut remain = *self;
                loop {
                    let digit = remain % BASE;
                    remain /= BASE;

                    offset -= 1;
                    // SAFETY: `remain` 会最终变为 0,并且循环会在 `offset`
                    // 回绕之前退出。
                    unsafe { core::hint::assert_unchecked(offset < buf.len()) }
                    buf[offset].write(DIG_TAB[digit as usize]);
                    if remain == 0 {
                        break;
                    }
                }

                // SAFETY: 从 `offset` 开始的切片元素都已被写入 ASCII 数字。
                let digits = unsafe { slice_buffer_to_str(&buf, offset) };
                f.pad_integral(true, $prefix, digits)
            }
        }

        #[stable(feature = "rust1", since = "1.0.0")]
        impl fmt::$Trait for $Signed {
            /// 按二进制补码位模式格式化有符号整数。
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::$Trait::fmt(&self.cast_unsigned(), f)
            }
        }
    };
}

/// 为所有整数类型生成非十进制基数的格式化实现。
macro_rules! radix_integers {
    ($Signed:ident, $Unsigned:ident) => {
        radix_integer! { fmt::Binary   for $Signed and $Unsigned, "0b", b"01" }
        radix_integer! { fmt::Octal    for $Signed and $Unsigned, "0o", b"01234567" }
        radix_integer! { fmt::LowerHex for $Signed and $Unsigned, "0x", b"0123456789abcdef" }
        radix_integer! { fmt::UpperHex for $Signed and $Unsigned, "0x", b"0123456789ABCDEF" }
    };
}
radix_integers! { isize, usize }
radix_integers! { i8, u8 }
radix_integers! { i16, u16 }
radix_integers! { i32, u32 }
radix_integers! { i64, u64 }
radix_integers! { i128, u128 }

macro_rules! impl_Debug {
    ($($T:ident)*) => {
        $(
            #[stable(feature = "rust1", since = "1.0.0")]
            impl fmt::Debug for $T {
                #[inline]
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    if f.debug_lower_hex() {
                        fmt::LowerHex::fmt(self, f)
                    } else if f.debug_upper_hex() {
                        fmt::UpperHex::fmt(self, f)
                    } else {
                        fmt::Display::fmt(self, f)
                    }
                }
            }
        )*
    };
}

// 由 00..99 范围内所有两位数字组成的字符串,用作十进制两位一组的查找表。
static DECIMAL_PAIRS: &[u8; 200] = b"\
      0001020304050607080910111213141516171819\
      2021222324252627282930313233343536373839\
      4041424344454647484950515253545556575859\
      6061626364656667686970717273747576777879\
      8081828384858687888990919293949596979899";

/// 从 `offset` 开始,把一段 ASCII 字节缓冲区转换为 `&str`。
///
/// # 安全性(Safety）
///
/// `buf` 从 `offset` 索引开始的内容必须已经初始化,并且必须只包含 ASCII 字符。
/// 本模块中的调用方通常通过查找表或 `b'0'` 偏移写入数字,因此 ASCII 同时保证
/// 结果也是合法 UTF-8。
unsafe fn slice_buffer_to_str(buf: &[MaybeUninit<u8>], offset: usize) -> &str {
    // SAFETY: `offset` 始终位于 0..=buf.len() 范围内。
    let written = unsafe { buf.get_unchecked(offset..) };
    // SAFETY: (`assume_init_ref`) 从 `offset` 开始的所有缓冲区内容都已初始化。
    // SAFETY: (`from_utf8_unchecked`) 写入只使用查找表中的 ASCII 字节。
    unsafe { str::from_utf8_unchecked(written.assume_init_ref()) }
}

macro_rules! impl_Display {
    ($($Signed:ident, $Unsigned:ident),* ; as $T:ident into $fmt_fn:ident) => {

        $(
        const _: () = {
            assert!($Signed::MIN < 0, "need signed");
            assert!($Unsigned::MIN == 0, "need unsigned");
            assert!($Signed::BITS == $Unsigned::BITS, "need counterparts");
            assert!($Signed::BITS <= $T::BITS, "need lossless conversion");
            assert!($Unsigned::BITS <= $T::BITS, "need lossless conversion");
        };

        #[stable(feature = "rust1", since = "1.0.0")]
        impl fmt::Display for $Unsigned {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                #[cfg(not(feature = "optimize_for_size"))]
                {
                    const MAX_DEC_N: usize = $Unsigned::MAX.ilog10() as usize + 1;
                    // 从缓冲区右侧向左写入 `self` 的十进制数字。
                    let mut buf = [MaybeUninit::<u8>::uninit(); MAX_DEC_N];

                    // SAFETY: `buf` 的容量始终足以容纳该整数的全部十进制数字。
                    unsafe { f.pad_integral(true, "", self._fmt(&mut buf)) }
                }
                #[cfg(feature = "optimize_for_size")]
                {
                    // 此宏顶部已经断言 `as` 转换不会丢失信息。
                    ${concat($fmt_fn, _small)}(*self as $T, true, f)
                }
            }
        }

        #[stable(feature = "rust1", since = "1.0.0")]
        impl fmt::Display for $Signed {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                #[cfg(not(feature = "optimize_for_size"))]
                {
                    const MAX_DEC_N: usize = $Unsigned::MAX.ilog10() as usize + 1;
                    // 从缓冲区右侧向左写入 `self` 的十进制数字。
                    let mut buf = [MaybeUninit::<u8>::uninit(); MAX_DEC_N];

                    // SAFETY: `buf` 的容量始终足以容纳该整数绝对值的全部十进制数字。
                    unsafe { f.pad_integral(*self >= 0, "", self.unsigned_abs()._fmt(&mut buf)) }
                }
                #[cfg(feature = "optimize_for_size")]
                {
                    // 此宏顶部已经断言 `as` 转换不会丢失信息。
                    return ${concat($fmt_fn, _small)}(self.unsigned_abs() as $T, *self >= 0, f);
                }
            }
        }

        #[cfg(not(feature = "optimize_for_size"))]
        impl $Unsigned {
            #[doc(hidden)]
            #[unstable(
                feature = "fmt_internals",
                reason = "specialized method meant to only be used by `SpecToString` implementation",
                issue = "none"
            )]
            pub unsafe fn _fmt<'a>(self, buf: &'a mut [MaybeUninit::<u8>]) -> &'a str {
                // SAFETY: 调用方提供的 `buf` 始终足以容纳全部十进制数字。
                let offset = unsafe { self._fmt_inner(buf) };
                // SAFETY: 从 `offset` 开始的切片元素都已被写入 ASCII 数字。
                unsafe { slice_buffer_to_str(buf, offset) }
            }

            unsafe fn _fmt_inner(self, buf: &mut [MaybeUninit::<u8>]) -> usize {
                // `offset` 记录 `buf` 前缀中尚未初始化的字节数。
                let mut offset = buf.len();
                // 从工作副本中逐步消耗最低有效的十进制位。
                let mut remain = self;

                // 借助查找表每次格式化四位数字。
                // 四位数字需要 16 位或更宽的 `$Unsigned`。
                while size_of::<Self>() > 1 && remain > 999.try_into().expect("branch is not hit for types that cannot fit 999 (u8)") {
                    // SAFETY: `MAX_DEC_N` 保证所有十进制数字都能放进 `buf`,
                    // 且 while 条件保证这里至少还会写入 4 位数字。
                    unsafe { core::hint::assert_unchecked(offset >= 4) }
                    // SAFETY: 由于上一条前置条件,`offset` 从初始的 `buf.len()`
                    // 向下递减时不会下溢。
                    unsafe { core::hint::assert_unchecked(offset <= buf.len()) }
                    offset -= 4;

                    // 每次取出两组两位数。
                    let scale: Self = 1_00_00.try_into().expect("branch is not hit for types that cannot fit 1E4 (u8)");
                    let quad = remain % scale;
                    remain /= scale;
                    let pair1 = (quad / 100) as usize;
                    let pair2 = (quad % 100) as usize;
                    buf[offset + 0].write(DECIMAL_PAIRS[pair1 * 2 + 0]);
                    buf[offset + 1].write(DECIMAL_PAIRS[pair1 * 2 + 1]);
                    buf[offset + 2].write(DECIMAL_PAIRS[pair2 * 2 + 0]);
                    buf[offset + 3].write(DECIMAL_PAIRS[pair2 * 2 + 1]);
                }

                // 借助查找表每次格式化两位数字。
                if remain > 9 {
                    // SAFETY: `MAX_DEC_N` 保证所有十进制数字都能放进 `buf`,
                    // 且 if 条件保证这里至少还会写入 2 位数字。
                    unsafe { core::hint::assert_unchecked(offset >= 2) }
                    // SAFETY: 由于上一条前置条件,`offset` 从初始的 `buf.len()`
                    // 向下递减时不会下溢。
                    unsafe { core::hint::assert_unchecked(offset <= buf.len()) }
                    offset -= 2;

                    let pair = (remain % 100) as usize;
                    remain /= 100;
                    buf[offset + 0].write(DECIMAL_PAIRS[pair * 2 + 0]);
                    buf[offset + 1].write(DECIMAL_PAIRS[pair * 2 + 1]);
                }

                // 如仍有最后一位数字,格式化它。
                if remain != 0 || self == 0 {
                    // SAFETY: `MAX_DEC_N` 保证所有十进制数字都能放进 `buf`,
                    // 且 if 条件保证这里至少还会写入 1 位数字。
                    unsafe { core::hint::assert_unchecked(offset >= 1) }
                    // SAFETY: 由于上一条前置条件,`offset` 从初始的 `buf.len()`
                    // 向下递减时不会下溢。
                    unsafe { core::hint::assert_unchecked(offset <= buf.len()) }
                    offset -= 1;

                    // 编译器要么能看出 `remain < 10`,要么会避免下一步产生边界检查。
                    let last = (remain & 15) as usize;
                    buf[offset].write(DECIMAL_PAIRS[last * 2 + 1]);
                    // 未使用: remain = 0;
                }

                offset
            }
        }

        impl $Signed {
            /// 允许调用者把整数的十进制文本写入其通过可变引用传入的 [`NumBuffer`]。
            ///
            /// 返回的 `&str` 借用自 `buf`,不会发生堆分配。对有符号类型而言,
            /// 负数会在数字前写入 `-`,正数不写入显式 `+`;额外符号处理仍由
            /// `Formatter` 在常规 `Display` 路径中负责。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(int_format_into)]
            /// use core::fmt::NumBuffer;
            ///
            #[doc = concat!("let n = 0", stringify!($Signed), ";")]
            /// let mut buf = NumBuffer::new();
            /// assert_eq!(n.format_into(&mut buf), "0");
            ///
            #[doc = concat!("let n1 = 32", stringify!($Signed), ";")]
            /// assert_eq!(n1.format_into(&mut buf), "32");
            ///
            #[doc = concat!("let n2 = ", stringify!($Signed::MAX), ";")]
            #[doc = concat!("assert_eq!(n2.format_into(&mut buf), ", stringify!($Signed::MAX), ".to_string());")]
            /// ```
            #[unstable(feature = "int_format_into", issue = "138215")]
            pub fn format_into(self, buf: &mut NumBuffer<Self>) -> &str {
                let mut offset;

                #[cfg(not(feature = "optimize_for_size"))]
                // SAFETY: `buf` 的容量始终足以容纳全部十进制数字。
                unsafe {
                    offset = self.unsigned_abs()._fmt_inner(&mut buf.buf);
                }
                #[cfg(feature = "optimize_for_size")]
                {
                    // 此宏顶部已经断言 `as` 转换不会丢失信息。
                    offset = ${concat($fmt_fn, _in_buf_small)}(self.unsigned_abs() as $T, &mut buf.buf);
                }
                // 有符号与无符号路径的差异只在于这 4 行负号处理。
                if self < 0 {
                    offset -= 1;
                    buf.buf[offset].write(b'-');
                }
                // SAFETY: 从 `offset` 开始的切片元素都已被写入 ASCII 字节。
                unsafe { slice_buffer_to_str(&buf.buf, offset) }
            }
        }

        impl $Unsigned {
            /// 允许调用者把整数的十进制文本写入其通过可变引用传入的 [`NumBuffer`]。
            ///
            /// 返回的 `&str` 借用自 `buf`,不会发生堆分配。该方法适合需要复用
            /// 栈上缓冲区的格式化热路径。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(int_format_into)]
            /// use core::fmt::NumBuffer;
            ///
            #[doc = concat!("let n = 0", stringify!($Unsigned), ";")]
            /// let mut buf = NumBuffer::new();
            /// assert_eq!(n.format_into(&mut buf), "0");
            ///
            #[doc = concat!("let n1 = 32", stringify!($Unsigned), ";")]
            /// assert_eq!(n1.format_into(&mut buf), "32");
            ///
            #[doc = concat!("let n2 = ", stringify!($Unsigned::MAX), ";")]
            #[doc = concat!("assert_eq!(n2.format_into(&mut buf), ", stringify!($Unsigned::MAX), ".to_string());")]
            /// ```
            #[unstable(feature = "int_format_into", issue = "138215")]
            pub fn format_into(self, buf: &mut NumBuffer<Self>) -> &str {
                let offset;

                #[cfg(not(feature = "optimize_for_size"))]
                // SAFETY: `buf` 的容量始终足以容纳全部十进制数字。
                unsafe {
                    offset = self._fmt_inner(&mut buf.buf);
                }
                #[cfg(feature = "optimize_for_size")]
                {
                    // 此宏顶部已经断言 `as` 转换不会丢失信息。
                    offset = ${concat($fmt_fn, _in_buf_small)}(self as $T, &mut buf.buf);
                }
                // SAFETY: 从 `offset` 开始的切片元素都已被写入 ASCII 字节。
                unsafe { slice_buffer_to_str(&buf.buf, offset) }
            }
        }

        )*

        #[cfg(feature = "optimize_for_size")]
        fn ${concat($fmt_fn, _in_buf_small)}(mut n: $T, buf: &mut [MaybeUninit::<u8>]) -> usize {
            let mut curr = buf.len();

            // SAFETY: 要说明写入 `buf_ptr` 是安全的,注意初始时
            // `curr == buf.len() == 39 > log(n)`,因为 `n < 2^128 < 10^39`。
            // 每一步除以 10 后都会维持这个关系。由于 `n` 始终非负,
            // 可知 `curr > 0`,因此可以安全访问 `buf_ptr[curr..curr + 1]`。
            loop {
                curr -= 1;
                buf[curr].write((n % 10) as u8 + b'0');
                n /= 10;

                if n == 0 {
                    break;
                }
            }
            curr
        }

        #[cfg(feature = "optimize_for_size")]
        fn ${concat($fmt_fn, _small)}(n: $T, is_nonnegative: bool, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            const MAX_DEC_N: usize = $T::MAX.ilog(10) as usize + 1;
            let mut buf = [MaybeUninit::<u8>::uninit(); MAX_DEC_N];

            let offset = ${concat($fmt_fn, _in_buf_small)}(n, &mut buf);
            // SAFETY: 从 `offset` 开始的切片元素都已被写入 ASCII 字节。
            let buf_slice = unsafe { slice_buffer_to_str(&buf, offset) };
            f.pad_integral(is_nonnegative, "", buf_slice)
        }
    };
}

macro_rules! impl_Exp {
    ($($Signed:ident, $Unsigned:ident),* ; as $T:ident into $fmt_fn:ident) => {
        const _: () = assert!($T::MIN == 0, "need unsigned");

        fn $fmt_fn(
            f: &mut fmt::Formatter<'_>,
            n: $T,
            is_nonnegative: bool,
            letter_e: u8
        ) -> fmt::Result {
            debug_assert!(letter_e.is_ascii_alphabetic(), "single-byte character");

            // 把整数打印成 (-10, 10) 范围内的系数。
            let mut exp = n.checked_ilog10().unwrap_or(0) as usize;
            debug_assert!(n / (10 as $T).pow(exp as u32) < 10);

            // precision 按小数部分的位数计算。
            let mut coef_prec = exp;
            // 把各位数字保留为整数,并用 `coef_prec` 记录小数位数。
            let mut coef = n;

            // `Formatter` 可能把 precision 设置为固定的小数位数。
            let more_prec = match f.precision() {
                None => {
                    // 省略所有尾随零。
                    while coef_prec != 0 && coef % 10 == 0 {
                        coef /= 10;
                        coef_prec -= 1;
                    }
                    0
                },

                Some(fmt_prec) if fmt_prec >= coef_prec => {
                    // 计算需要额外补多少个零。
                    fmt_prec - coef_prec
                },

                Some(fmt_prec) => {
                    // 计算需要丢弃多少位数字。
                    let less_prec = coef_prec - fmt_prec;
                    assert!(less_prec > 0);
                    // 缩小系数/precision 这一对值。例如 coef 123456 配合
                    // coef_prec 5 表示 1.23456。若要用 2 位小数格式化,
                    // 即 fmt_prec 为 2,则应按 10⁵⁻²=1000 缩放,得到
                    // coef 123 与 coef_prec 2。

                    // SAFETY: 任何小于 coef_prec 的 precision 都会得到一个
                    // 小于 coef 值的 10 的幂。
                    let scale = unsafe {
                        (10 as $T).checked_pow(less_prec as u32).unwrap_unchecked()
                    };
                    let floor = coef / scale;
                    // 按文档约定执行 round half to even。
                    let over = coef % scale;
                    let half = scale / 2;
                    let round_up = if over < half {
                        0
                    } else if over > half {
                        1
                    } else {
                        floor & 1 // 奇数向偶数舍入
                    };
                    // 缩放因子至少为 10 时,加 1 不会溢出。
                    coef = floor + round_up;
                    coef_prec = fmt_prec;

                    // round_up 可能让系数达到 10,而这是不允许的。例如
                    // [9.95, 10) 范围内的任意值在调整到 precision 1 时都会变成 10.0。
                    if round_up != 0 && coef.checked_ilog10().unwrap_or(0) as usize > coef_prec {
                        debug_assert_eq!(coef, (10 as $T).pow(coef_prec as u32 + 1));
                        coef /= 10; // 丢弃一个尾随零
                        exp += 1;   // 指数提高一个 10 的幂
                    }
                    0
                },
            };

            // 分配一个延迟初始化的文本缓冲区。
            const MAX_DEC_N: usize = $T::MAX.ilog10() as usize + 1;
            const MAX_COEF_LEN: usize = MAX_DEC_N + ".".len();
            const MAX_TEXT_LEN: usize = MAX_COEF_LEN + "e99".len();
            let mut buf = [MaybeUninit::<u8>::uninit(); MAX_TEXT_LEN];

            // 把系数编码到 buf[..coef_len]。
            let (lead_dec, coef_len) = if coef_prec == 0 && more_prec == 0 {
                (coef, 1_usize) // 单个数字;没有小数部分
            } else {
                buf[1].write(b'.');
                let fraction_range = 2..(2 + coef_prec);

                // 从工作副本中逐步消耗最低有效的十进制位。
                let mut remain = coef;
                #[cfg(feature = "optimize_for_size")] {
                    for i in fraction_range.clone().rev() {
                        let digit = (remain % 10) as usize;
                        remain /= 10;
                        buf[i].write(b'0' + digit as u8);
                    }
                }
                #[cfg(not(feature = "optimize_for_size"))] {
                    // 借助查找表每次写入两位数字。
                    for i in fraction_range.clone().skip(1).rev().step_by(2) {
                        let pair = (remain % 100) as usize;
                        remain /= 100;
                        buf[i - 1].write(DECIMAL_PAIRS[pair * 2 + 0]);
                        buf[i - 0].write(DECIMAL_PAIRS[pair * 2 + 1]);
                    }
                    // 奇数个数字会留下最后一位需要单独写入。
                    if coef_prec & 1 != 0 {
                        let digit = (remain % 10) as usize;
                        remain /= 10;
                        buf[fraction_range.start].write(b'0' + digit as u8);
                    }
                }

                (remain, fraction_range.end)
            };
            debug_assert!(lead_dec < 10);
            debug_assert!(lead_dec != 0 || coef == 0, "significant digits only");
            buf[0].write(b'0' + lead_dec as u8);

            // SAFETY: 十进制位数受到 MAX 上界限制。
            unsafe { core::hint::assert_unchecked(coef_len <= MAX_COEF_LEN) }
            // 把指数部分编码到 buf[coef_len..text_len]。
            buf[coef_len].write(letter_e);
            let text_len: usize = match exp {
                ..10 => {
                    buf[coef_len + 1].write(b'0' + exp as u8);
                    coef_len + 2
                },
                10..100 => {
                    #[cfg(feature = "optimize_for_size")] {
                        buf[coef_len + 1].write(b'0' + (exp / 10) as u8);
                        buf[coef_len + 2].write(b'0' + (exp % 10) as u8);
                    }
                    #[cfg(not(feature = "optimize_for_size"))] {
                        buf[coef_len + 1].write(DECIMAL_PAIRS[exp * 2 + 0]);
                        buf[coef_len + 2].write(DECIMAL_PAIRS[exp * 2 + 1]);
                    }
                    coef_len + 3
                },
                _ => {
                    const { assert!($T::MAX.ilog10() < 100) };
                    // SAFETY: 即使是 `u256::MAX`,指数也只有 77。
                    unsafe { core::hint::unreachable_unchecked() }
                }
            };
            // SAFETY: text_len 之前的所有字节都已写入。
            let text = unsafe { buf[..text_len].assume_init_ref() };

            if more_prec == 0 {
                // SAFETY: text 只包含 ASCII:十进制数字、LETTER_E 或点号。
                // ASCII 蕴含合法 UTF-8。
                let as_str = unsafe { str::from_utf8_unchecked(text) };
                f.pad_integral(is_nonnegative, "", as_str)
            } else {
                let parts = &[
                    numfmt::Part::Copy(&text[..coef_len]),
                    numfmt::Part::Zero(more_prec),
                    numfmt::Part::Copy(&text[coef_len..]),
                ];
                let sign = if !is_nonnegative {
                    "-"
                } else if f.sign_plus() {
                    "+"
                } else {
                    ""
                };
                // SAFETY: text 只包含 ASCII:十进制数字、LETTER_E 或点号。
                // ASCII 蕴含合法 UTF-8。
                unsafe { f.pad_formatted_parts(&numfmt::Formatted { sign, parts }) }
            }
        }

        $(
        const _: () = {
            assert!($Signed::MIN < 0, "need signed");
            assert!($Unsigned::MIN == 0, "need unsigned");
            assert!($Signed::BITS == $Unsigned::BITS, "need counterparts");
            assert!($Signed::BITS <= $T::BITS, "need lossless conversion");
            assert!($Unsigned::BITS <= $T::BITS, "need lossless conversion");
        };
        #[stable(feature = "integer_exp_format", since = "1.42.0")]
        impl fmt::LowerExp for $Signed {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                $fmt_fn(f, self.unsigned_abs() as $T, *self >= 0, b'e')
            }
        }
        #[stable(feature = "integer_exp_format", since = "1.42.0")]
        impl fmt::LowerExp for $Unsigned {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                $fmt_fn(f, *self as $T, true, b'e')
            }
        }
        #[stable(feature = "integer_exp_format", since = "1.42.0")]
        impl fmt::UpperExp for $Signed {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                $fmt_fn(f, self.unsigned_abs() as $T, *self >= 0, b'E')
            }
        }
        #[stable(feature = "integer_exp_format", since = "1.42.0")]
        impl fmt::UpperExp for $Unsigned {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                $fmt_fn(f, *self as $T, true, b'E')
            }
        }
        )*

    };
}

impl_Debug! {
    i8 i16 i32 i64 i128 isize
    u8 u16 u32 u64 u128 usize
}

// 把 wasm32 纳入这里,因为它不能代表本机指针宽度,并且通常很在意更小的代码体积。
#[cfg(any(target_pointer_width = "64", target_arch = "wasm32"))]
mod imp {
    use super::*;
    impl_Display!(i8, u8, i16, u16, i32, u32, i64, u64, isize, usize; as u64 into display_u64);
    impl_Exp!(i8, u8, i16, u16, i32, u32, i64, u64, isize, usize; as u64 into exp_u64);
}

#[cfg(not(any(target_pointer_width = "64", target_arch = "wasm32")))]
mod imp {
    use super::*;
    impl_Display!(i8, u8, i16, u16, i32, u32, isize, usize; as u32 into display_u32);
    impl_Display!(i64, u64; as u64 into display_u64);

    impl_Exp!(i8, u8, i16, u16, i32, u32, isize, usize; as u32 into exp_u32);
    impl_Exp!(i64, u64; as u64 into exp_u64);
}
impl_Exp!(i128, u128; as u128 into exp_u128);

const U128_MAX_DEC_N: usize = u128::MAX.ilog10() as usize + 1;

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for u128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [MaybeUninit::<u8>::uninit(); U128_MAX_DEC_N];

        // SAFETY: `buf` 的容量始终足以容纳全部十进制数字。
        unsafe { f.pad_integral(true, "", self._fmt(&mut buf)) }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for i128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 这不是笔误:这里使用 `u128` 的最大数字位数,因此使用 `U128_MAX_DEC_N`。
        let mut buf = [MaybeUninit::<u8>::uninit(); U128_MAX_DEC_N];

        let is_nonnegative = *self >= 0;
        // SAFETY: `buf` 的容量始终足以容纳绝对值的全部十进制数字。
        unsafe { f.pad_integral(is_nonnegative, "", self.unsigned_abs()._fmt(&mut buf)) }
    }
}

impl u128 {
    /// 针对 u128 优化的格式化实现。
    ///
    /// 128 位计算成本较高,因此这里每次按 16 个十进制位为一批处理,减少大整数
    /// 除法和取余的次数。
    #[doc(hidden)]
    #[unstable(
        feature = "fmt_internals",
        reason = "specialized method meant to only be used by `SpecToString` implementation",
        issue = "none"
    )]
    pub unsafe fn _fmt<'a>(self, buf: &'a mut [MaybeUninit<u8>]) -> &'a str {
        // SAFETY: 调用方提供的 `buf` 始终足以容纳全部十进制数字。
        let offset = unsafe { self._fmt_inner(buf) };
        // SAFETY: 从 `offset` 开始的切片元素都已被写入 ASCII 数字。
        unsafe { slice_buffer_to_str(buf, offset) }
    }

    unsafe fn _fmt_inner(self, buf: &mut [MaybeUninit<u8>]) -> usize {
        // 优化常见的零值;由于它的“前导”零语义特殊,本来也需要单独处理。
        if self == 0 {
            let offset = buf.len() - 1;
            buf[offset].write(b'0');
            return offset;
        }
        // 取最低有效的 16 个十进制位。
        let (quot_1e16, mod_1e16) = div_rem_1e16(self);
        let (mut remain, mut offset) = if quot_1e16 == 0 {
            (mod_1e16, U128_MAX_DEC_N)
        } else {
            // 写入 buf[23..39] 范围内的数字。
            enc_16lsd::<{ U128_MAX_DEC_N - 16 }>(buf, mod_1e16);

            // 再取 16 个十进制位。
            let (quot2, mod2) = div_rem_1e16(quot_1e16);
            if quot2 == 0 {
                (mod2, U128_MAX_DEC_N - 16)
            } else {
                // 写入 buf[7..23] 范围内的数字。
                enc_16lsd::<{ U128_MAX_DEC_N - 32 }>(buf, mod2);
                // 两次除以 1e16 后,quot2 最多还剩 7 个十进制位。
                (quot2 as u64, U128_MAX_DEC_N - 32)
            }
        };

        // 借助查找表每次格式化四位数字。
        while remain > 999 {
            // SAFETY: `U128_MAX_DEC_N` 保证所有十进制数字都能放进 `buf`,
            // 且 while 条件保证这里至少还会写入 4 位数字。
            unsafe { core::hint::assert_unchecked(offset >= 4) }
            // SAFETY: 由于上一条前置条件,`offset` 从初始的 `buf.len()`
            // 向下递减时不会下溢。
            unsafe { core::hint::assert_unchecked(offset <= buf.len()) }
            offset -= 4;

            // 每次取出两组两位数。
            let quad = remain % 1_00_00;
            remain /= 1_00_00;
            let pair1 = (quad / 100) as usize;
            let pair2 = (quad % 100) as usize;
            buf[offset + 0].write(DECIMAL_PAIRS[pair1 * 2 + 0]);
            buf[offset + 1].write(DECIMAL_PAIRS[pair1 * 2 + 1]);
            buf[offset + 2].write(DECIMAL_PAIRS[pair2 * 2 + 0]);
            buf[offset + 3].write(DECIMAL_PAIRS[pair2 * 2 + 1]);
        }

        // 借助查找表每次格式化两位数字。
        if remain > 9 {
            // SAFETY: `U128_MAX_DEC_N` 保证所有十进制数字都能放进 `buf`,
            // 且 if 条件保证这里至少还会写入 2 位数字。
            unsafe { core::hint::assert_unchecked(offset >= 2) }
            // SAFETY: 由于上一条前置条件,`offset` 从初始的 `buf.len()`
            // 向下递减时不会下溢。
            unsafe { core::hint::assert_unchecked(offset <= buf.len()) }
            offset -= 2;

            let pair = (remain % 100) as usize;
            remain /= 100;
            buf[offset + 0].write(DECIMAL_PAIRS[pair * 2 + 0]);
            buf[offset + 1].write(DECIMAL_PAIRS[pair * 2 + 1]);
        }

        // 如仍有最后一位数字,格式化它。
        if remain != 0 {
            // SAFETY: `U128_MAX_DEC_N` 保证所有十进制数字都能放进 `buf`,
            // 且 if 条件保证这里至少还会写入 1 位数字。
            unsafe { core::hint::assert_unchecked(offset >= 1) }
            // SAFETY: 由于上一条前置条件,`offset` 从初始的 `buf.len()`
            // 向下递减时不会下溢。
            unsafe { core::hint::assert_unchecked(offset <= buf.len()) }
            offset -= 1;

            // 编译器要么能看出 `remain < 10`,要么会避免下一步产生边界检查。
            let last = (remain & 15) as usize;
            buf[offset].write(DECIMAL_PAIRS[last * 2 + 1]);
            // 未使用: remain = 0;
        }
        offset
    }

    /// 允许调用者把 `u128` 的十进制文本写入其通过可变引用传入的 [`NumBuffer`]。
    ///
    /// 返回的 `&str` 借用自 `buf`,不会发生堆分配。`u128` 使用专门的分批格式化
    /// 路径,以避免在热路径上反复执行完整 128 位除法。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(int_format_into)]
    /// use core::fmt::NumBuffer;
    ///
    /// let n = 0u128;
    /// let mut buf = NumBuffer::new();
    /// assert_eq!(n.format_into(&mut buf), "0");
    ///
    /// let n1 = 32u128;
    /// let mut buf1 = NumBuffer::new();
    /// assert_eq!(n1.format_into(&mut buf1), "32");
    ///
    /// let n2 = u128::MAX;
    /// let mut buf2 = NumBuffer::new();
    /// assert_eq!(n2.format_into(&mut buf2), u128::MAX.to_string());
    /// ```
    #[unstable(feature = "int_format_into", issue = "138215")]
    pub fn format_into(self, buf: &mut NumBuffer<Self>) -> &str {
        let diff = buf.capacity() - U128_MAX_DEC_N;
        // FIXME: 等 const generics 能更好支持后,把 `NumberBufferTrait::BUF_SIZE`
        // 作为 `fmt_u128_inner` 的泛型 const 使用。
        //
        // 在此之前,必须使用从索引 1 开始的切片,并给返回的 offset 加 1,
        // 以确保数字正确生成在缓冲区末尾。
        // SAFETY: `diff` 始终位于 0 和其初始值之间。
        unsafe { self._fmt(buf.buf.get_unchecked_mut(diff..)) }
    }
}

impl i128 {
    /// 允许调用者把 `i128` 的十进制文本写入其通过可变引用传入的 [`NumBuffer`]。
    ///
    /// 返回的 `&str` 借用自 `buf`,不会发生堆分配。负数会先格式化其无符号绝对值,
    /// 再在结果前补写 `-`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(int_format_into)]
    /// use core::fmt::NumBuffer;
    ///
    /// let n = 0i128;
    /// let mut buf = NumBuffer::new();
    /// assert_eq!(n.format_into(&mut buf), "0");
    ///
    /// let n1 = i128::MIN;
    /// assert_eq!(n1.format_into(&mut buf), i128::MIN.to_string());
    ///
    /// let n2 = i128::MAX;
    /// assert_eq!(n2.format_into(&mut buf), i128::MAX.to_string());
    /// ```
    #[unstable(feature = "int_format_into", issue = "138215")]
    pub fn format_into(self, buf: &mut NumBuffer<Self>) -> &str {
        let diff = buf.capacity() - U128_MAX_DEC_N;
        // FIXME: 等 const generics 能更好支持后,把 `NumberBufferTrait::BUF_SIZE`
        // 作为 `fmt_u128_inner` 的泛型 const 使用。
        //
        // 在此之前,必须使用从索引 1 开始的切片,并给返回的 offset 加 1,
        // 以确保数字正确生成在缓冲区末尾。
        let mut offset =
            // SAFETY: `buf` 的容量始终足以容纳全部十进制数字。
            unsafe { self.unsigned_abs()._fmt_inner(buf.buf.get_unchecked_mut(diff..)) };
        // 把 offset 调回相对于原始缓冲区的位置。
        offset += diff;
        // 有符号与无符号路径的差异只在于这 4 行负号处理。
        if self < 0 {
            offset -= 1;
            // SAFETY: `buf` 的容量始终足以容纳全部数字和负号。
            unsafe {
                buf.buf.get_unchecked_mut(offset).write(b'-');
            }
        }
        // SAFETY: 从 `offset` 开始的切片元素都已被写入 ASCII 字节。
        unsafe { slice_buffer_to_str(&buf.buf, offset) }
    }
}

/// 把 `n` 的最低有效 16 个十进制位编码到 `buf[OFFSET .. OFFSET + 16]`。
fn enc_16lsd<const OFFSET: usize>(buf: &mut [MaybeUninit<u8>], n: u64) {
    // 从工作副本中逐步消耗最低有效的十进制位。
    let mut remain = n;

    // 借助查找表每次格式化四位数字。
    for quad_index in (0..4).rev() {
        // 每次取出两组两位数。
        let quad = remain % 1_00_00;
        remain /= 1_00_00;
        let pair1 = (quad / 100) as usize;
        let pair2 = (quad % 100) as usize;
        buf[quad_index * 4 + OFFSET + 0].write(DECIMAL_PAIRS[pair1 * 2 + 0]);
        buf[quad_index * 4 + OFFSET + 1].write(DECIMAL_PAIRS[pair1 * 2 + 1]);
        buf[quad_index * 4 + OFFSET + 2].write(DECIMAL_PAIRS[pair2 * 2 + 0]);
        buf[quad_index * 4 + OFFSET + 3].write(DECIMAL_PAIRS[pair2 * 2 + 1]);
    }
}

/// 用常数 1E16 执行欧几里得除法并同时得到余数,本质上一次消耗 `n` 的
/// 16 个十进制位。
///
/// 这里的整数除法算法基于以下论文:
///
///   T. Granlund and P. Montgomery, “Division by Invariant Integers Using Multiplication”
///   in Proc. of the SIGPLAN94 Conference on Programming Language Design and
///   Implementation, 1994, pp. 61–72
///
#[inline]
fn div_rem_1e16(n: u128) -> (u128, u64) {
    const D: u128 = 1_0000_0000_0000_0000;
    // 这个检查能很好地内联进调用方控制流。
    if n < D {
        return (0, n as u64);
    }

    // 这些常量值使用 Granlund 与 Montgomery 论文中的 CHOOSE_MULTIPLIER
    // 过程计算而来,其中 N=128、prec=128、d=1E16。
    const M_HIGH: u128 = 76624777043294442917917351357515459181;
    const SH_POST: u8 = 51;

    let quot = n.widening_mul(M_HIGH).1 >> SH_POST;
    let rem = n - quot * D;
    (quot, rem as u64)
}
