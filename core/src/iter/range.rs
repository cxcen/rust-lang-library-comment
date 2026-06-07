use super::{
    FusedIterator, TrustedLen, TrustedRandomAccess, TrustedRandomAccessNoCoerce, TrustedStep,
};
use crate::ascii::Char as AsciiChar;
use crate::mem;
use crate::net::{Ipv4Addr, Ipv6Addr};
use crate::num::NonZero;
use crate::ops::{self, Try};

// SAFETY: 这些基本标量类型的 `Step` 实现维护了全部不变量。
macro_rules! unsafe_impl_trusted_step {
    ($($type:ty)*) => {$(
        #[unstable(feature = "trusted_step", issue = "85731")]
        unsafe impl TrustedStep for $type {}
    )*};
}
unsafe_impl_trusted_step![AsciiChar char i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize Ipv4Addr Ipv6Addr];

/// 具有“后继”和“前驱”操作概念的对象。
///
/// `Step` 是 range 迭代的基础协议: 它定义如何从一个值向比较结果更大的方向前进，
/// 以及如何向比较结果更小的方向后退。整数、`char`、IP 地址等类型可以用它描述
/// 半开或闭合 range 中相邻元素之间的步进关系。
///
/// *后继*操作朝更大的值移动。*前驱*操作朝更小的值移动。所有方法的不变量共同保证
/// range 的 `Iterator::size_hint`、`ExactSizeIterator`、`TrustedLen` 以及
/// `DoubleEndedIterator` 实现可以把“步数”当作剩余长度来使用。
#[rustc_diagnostic_item = "range_step"]
#[rustc_on_unimplemented(
    message = "`std::ops::Range<{Self}>` is not an iterator",
    label = "`Range<{Self}>` is not an iterator",
    note = "`Range` only implements `Iterator` for select types in the standard library, \
            particularly integers; to see the full list of types, see the documentation for the \
            unstable `Step` trait"
)]
#[unstable(feature = "step_trait", issue = "42168")]
pub trait Step: Clone + PartialOrd + Sized {
    /// 返回从 `start` 到 `end` 所需*后继*步数的上下界，形式类似
    /// [`Iterator::size_hint()`][Iterator::size_hint()]。
    ///
    /// 如果步数会溢出 `usize`，或步数是无限的，则返回 `(usize::MAX, None)`。
    ///
    /// # 不变量
    ///
    /// 对任意 `a`、`b` 和 `n`:
    ///
    /// * `steps_between(&a, &b) == (n, Some(n))` 当且仅当 `Step::forward_checked(&a, n) == Some(b)`
    /// * `steps_between(&a, &b) == (n, Some(n))` 当且仅当 `Step::backward_checked(&b, n) == Some(a)`
    /// * 只有在 `a <= b` 时，才有 `steps_between(&a, &b) == (n, Some(n))`
    ///   * 推论: `steps_between(&a, &b) == (0, Some(0))` 当且仅当 `a == b`
    /// * 如果 `a > b`，则 `steps_between(&a, &b) == (0, None)`
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>);

    /// 返回对 `start` 连续执行 `count` 次*后继*操作后得到的值。
    ///
    /// 如果这会溢出 `Self` 支持的取值范围，则返回 `None`。
    ///
    /// # 不变量
    ///
    /// 对任意 `a`、`n` 和 `m`:
    ///
    /// * `Step::forward_checked(a, n).and_then(|x| Step::forward_checked(x, m)) == Step::forward_checked(a, m).and_then(|x| Step::forward_checked(x, n))`
    /// * `Step::forward_checked(a, n).and_then(|x| Step::forward_checked(x, m)) == try { Step::forward_checked(a, n.checked_add(m)) }`
    ///
    /// 对任意 `a` 和 `n`:
    ///
    /// * `Step::forward_checked(a, n) == (0..n).try_fold(a, |x, _| Step::forward_checked(&x, 1))`
    ///   * 推论: `Step::forward_checked(a, 0) == Some(a)`
    fn forward_checked(start: Self, count: usize) -> Option<Self>;

    /// 返回对 `start` 连续执行 `count` 次*后继*操作后得到的值。
    ///
    /// 如果这会溢出 `Self` 支持的取值范围，本函数允许 panic、回绕或饱和。
    /// 建议行为是在启用 debug assertion 时 panic，在其他构建中回绕或饱和。
    ///
    /// unsafe 代码不能依赖溢出后的具体行为是否正确。
    ///
    /// # 不变量
    ///
    /// 对任意不会发生溢出的 `a`、`n` 和 `m`:
    ///
    /// * `Step::forward(Step::forward(a, n), m) == Step::forward(a, n + m)`
    ///
    /// 对任意不会发生溢出的 `a` 和 `n`:
    ///
    /// * `Step::forward_checked(a, n) == Some(Step::forward(a, n))`
    /// * `Step::forward(a, n) == (0..n).fold(a, |x, _| Step::forward(x, 1))`
    ///   * 推论: `Step::forward(a, 0) == a`
    /// * `Step::forward(a, n) >= a`
    /// * `Step::backward(Step::forward(a, n), n) == a`
    fn forward(start: Self, count: usize) -> Self {
        Step::forward_checked(start, count).expect("overflow in `Step::forward`")
    }

    /// 返回对 `start` 连续执行 `count` 次*后继*操作后得到的值，不做溢出检查。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须保证该操作不会溢出 `Self` 支持的取值范围。若发生溢出，本操作会
    /// 造成未定义行为。无法证明不溢出时，应改用 `forward` 或 `forward_checked`。
    ///
    /// # 不变量
    ///
    /// 对任意 `a`:
    ///
    /// * 如果存在 `b` 使得 `b > a`，则调用 `Step::forward_unchecked(a, 1)` 是安全的。
    /// * 如果存在 `b`、`n` 使得 `steps_between(&a, &b) == Some(n)`，
    ///   则对任意 `m <= n` 调用 `Step::forward_unchecked(a, m)` 都是安全的。
    ///   * 推论: `Step::forward_unchecked(a, 0)` 始终安全。
    ///
    /// 对任意不会发生溢出的 `a` 和 `n`:
    ///
    /// * `Step::forward_unchecked(a, n)` 等价于 `Step::forward(a, n)`
    unsafe fn forward_unchecked(start: Self, count: usize) -> Self {
        Step::forward(start, count)
    }

    /// 返回对 `start` 连续执行 `count` 次*前驱*操作后得到的值。
    ///
    /// 如果这会溢出 `Self` 支持的取值范围，则返回 `None`。
    ///
    /// # 不变量
    ///
    /// 对任意 `a`、`n` 和 `m`:
    ///
    /// * `Step::backward_checked(a, n).and_then(|x| Step::backward_checked(x, m)) == n.checked_add(m).and_then(|x| Step::backward_checked(a, x))`
    /// * `Step::backward_checked(a, n).and_then(|x| Step::backward_checked(x, m)) == try { Step::backward_checked(a, n.checked_add(m)?) }`
    ///
    /// 对任意 `a` 和 `n`:
    ///
    /// * `Step::backward_checked(a, n) == (0..n).try_fold(a, |x, _| Step::backward_checked(x, 1))`
    ///   * 推论: `Step::backward_checked(a, 0) == Some(a)`
    fn backward_checked(start: Self, count: usize) -> Option<Self>;

    /// 返回对 `start` 连续执行 `count` 次*前驱*操作后得到的值。
    ///
    /// 如果这会溢出 `Self` 支持的取值范围，本函数允许 panic、回绕或饱和。
    /// 建议行为是在启用 debug assertion 时 panic，在其他构建中回绕或饱和。
    ///
    /// unsafe 代码不能依赖溢出后的具体行为是否正确。
    ///
    /// # 不变量
    ///
    /// 对任意不会发生溢出的 `a`、`n` 和 `m`:
    ///
    /// * `Step::backward(Step::backward(a, n), m) == Step::backward(a, n + m)`
    ///
    /// 对任意不会发生溢出的 `a` 和 `n`:
    ///
    /// * `Step::backward_checked(a, n) == Some(Step::backward(a, n))`
    /// * `Step::backward(a, n) == (0..n).fold(a, |x, _| Step::backward(x, 1))`
    ///   * 推论: `Step::backward(a, 0) == a`
    /// * `Step::backward(a, n) <= a`
    /// * `Step::forward(Step::backward(a, n), n) == a`
    fn backward(start: Self, count: usize) -> Self {
        Step::backward_checked(start, count).expect("overflow in `Step::backward`")
    }

    /// 返回对 `start` 连续执行 `count` 次*前驱*操作后得到的值，不做溢出检查。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须保证该操作不会溢出 `Self` 支持的取值范围。若发生溢出，本操作会
    /// 造成未定义行为。无法证明不溢出时，应改用 `backward` 或 `backward_checked`。
    ///
    /// # 不变量
    ///
    /// 对任意 `a`:
    ///
    /// * 如果存在 `b` 使得 `b < a`，则调用 `Step::backward_unchecked(a, 1)` 是安全的。
    /// * 如果存在 `b`、`n` 使得 `steps_between(&b, &a) == (n, Some(n))`，
    ///   则对任意 `m <= n` 调用 `Step::backward_unchecked(a, m)` 都是安全的。
    ///   * 推论: `Step::backward_unchecked(a, 0)` 始终安全。
    ///
    /// 对任意不会发生溢出的 `a` 和 `n`:
    ///
    /// * `Step::backward_unchecked(a, n)` 等价于 `Step::backward(a, n)`
    unsafe fn backward_unchecked(start: Self, count: usize) -> Self {
        Step::backward(start, count)
    }
}

// 有符号 range 使用单独实现，因为有符号范围内的距离可能大于 signed::MAX。
// 因此直接用 `as` 转成有符号类型是不正确的。
macro_rules! step_signed_methods {
    ($unsigned: ty) => {
        #[inline]
        unsafe fn forward_unchecked(start: Self, n: usize) -> Self {
            // SAFETY: 调用方必须保证 `start + n` 不溢出。
            unsafe { start.checked_add_unsigned(n as $unsigned).unwrap_unchecked() }
        }

        #[inline]
        unsafe fn backward_unchecked(start: Self, n: usize) -> Self {
            // SAFETY: 调用方必须保证 `start - n` 不溢出。
            unsafe { start.checked_sub_unsigned(n as $unsigned).unwrap_unchecked() }
        }
    };
}

macro_rules! step_unsigned_methods {
    () => {
        #[inline]
        unsafe fn forward_unchecked(start: Self, n: usize) -> Self {
            // SAFETY: 调用方必须保证 `start + n` 不溢出。
            unsafe { start.unchecked_add(n as Self) }
        }

        #[inline]
        unsafe fn backward_unchecked(start: Self, n: usize) -> Self {
            // SAFETY: 调用方必须保证 `start - n` 不溢出。
            unsafe { start.unchecked_sub(n as Self) }
        }
    };
}

// 这些方法仍由宏生成，因为整数字面量会解析为不同类型。
macro_rules! step_identical_methods {
    () => {
        #[inline]
        #[allow(arithmetic_overflow)]
        #[rustc_inherit_overflow_checks]
        fn forward(start: Self, n: usize) -> Self {
            // 在 debug 构建中，溢出时触发 panic。
            // 在 release 构建中，这应当会被完全优化掉。
            if Self::forward_checked(start, n).is_none() {
                let _ = Self::MAX + 1;
            }
            // 使用 wrapping 运算以允许例如 `Step::forward(-128i8, 255)`。
            start.wrapping_add(n as Self)
        }

        #[inline]
        #[allow(arithmetic_overflow)]
        #[rustc_inherit_overflow_checks]
        fn backward(start: Self, n: usize) -> Self {
            // 在 debug 构建中，溢出时触发 panic。
            // 在 release 构建中，这应当会被完全优化掉。
            if Self::backward_checked(start, n).is_none() {
                let _ = Self::MIN - 1;
            }
            // 使用 wrapping 运算以允许例如 `Step::backward(127i8, 255)`。
            start.wrapping_sub(n as Self)
        }
    };
}

macro_rules! step_integer_impls {
    {
        narrower than or same width as usize:
            $( [ $u_narrower:ident $i_narrower:ident ] ),+;
        wider than usize:
            $( [ $u_wider:ident $i_wider:ident ] ),+;
    } => {
        $(
            #[allow(unreachable_patterns)]
            #[unstable(feature = "step_trait", issue = "42168")]
            impl Step for $u_narrower {
                step_identical_methods!();
                step_unsigned_methods!();

                #[inline]
                fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
                    if *start <= *end {
                        // 这里依赖 $u_narrower <= usize。
                        let steps = (*end - *start) as usize;
                        (steps, Some(steps))
                    } else {
                        (0, None)
                    }
                }

                #[inline]
                fn forward_checked(start: Self, n: usize) -> Option<Self> {
                    match Self::try_from(n) {
                        Ok(n) => start.checked_add(n),
                        Err(_) => None, // 如果 n 超出范围，`unsigned_start + n` 也会超出
                    }
                }

                #[inline]
                fn backward_checked(start: Self, n: usize) -> Option<Self> {
                    match Self::try_from(n) {
                        Ok(n) => start.checked_sub(n),
                        Err(_) => None, // 如果 n 超出范围，`unsigned_start - n` 也会超出
                    }
                }
            }

            #[allow(unreachable_patterns)]
            #[unstable(feature = "step_trait", issue = "42168")]
            impl Step for $i_narrower {
                step_identical_methods!();
                step_signed_methods!($u_narrower);

                #[inline]
                fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
                    if *start <= *end {
                        // 这里依赖 $i_narrower <= usize。
                        //
                        // 转换为 isize 会扩展宽度但保留符号。
                        // 在 isize 空间中使用 wrapping_sub，再转换为 usize，以计算可能
                        // 无法放入 isize 取值范围的差值。
                        let steps = (*end as isize).wrapping_sub(*start as isize) as usize;
                        (steps, Some(steps))
                    } else {
                        (0, None)
                    }
                }

                #[inline]
                fn forward_checked(start: Self, n: usize) -> Option<Self> {
                    match $u_narrower::try_from(n) {
                        Ok(n) => {
                            // Wrapping 可处理类似这样的情况:
                            // `Step::forward(-120_i8, 200) == Some(80_i8)`,
                            // 即使 200 超出了 i8 的范围。
                            let wrapped = start.wrapping_add(n as Self);
                            if wrapped >= start {
                                Some(wrapped)
                            } else {
                                None // 加法溢出
                            }
                        }
                        // 如果 n 超出了例如 u8 的范围，
                        // 那它就大于 i8 整个取值范围的宽度，
                        // 因此 `any_i8 + n` 必然使 i8 溢出。
                        Err(_) => None,
                    }
                }

                #[inline]
                fn backward_checked(start: Self, n: usize) -> Option<Self> {
                    match $u_narrower::try_from(n) {
                        Ok(n) => {
                            // Wrapping 可处理类似这样的情况:
                            // `Step::forward(-120_i8, 200) == Some(80_i8)`,
                            // 即使 200 超出了 i8 的范围。
                            let wrapped = start.wrapping_sub(n as Self);
                            if wrapped <= start {
                                Some(wrapped)
                            } else {
                                None // 减法溢出
                            }
                        }
                        // 如果 n 超出了例如 u8 的范围，
                        // 那它就大于 i8 整个取值范围的宽度，
                        // 因此 `any_i8 - n` 必然使 i8 溢出。
                        Err(_) => None,
                    }
                }
            }
        )+

        $(
            #[allow(unreachable_patterns)]
            #[unstable(feature = "step_trait", issue = "42168")]
            impl Step for $u_wider {
                step_identical_methods!();
                step_unsigned_methods!();

                #[inline]
                fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
                    if *start <= *end {
                        if let Ok(steps) = usize::try_from(*end - *start) {
                            (steps, Some(steps))
                        } else {
                            (usize::MAX, None)
                        }
                    } else {
                        (0, None)
                    }
                }

                #[inline]
                fn forward_checked(start: Self, n: usize) -> Option<Self> {
                    start.checked_add(n as Self)
                }

                #[inline]
                fn backward_checked(start: Self, n: usize) -> Option<Self> {
                    start.checked_sub(n as Self)
                }
            }

            #[allow(unreachable_patterns)]
            #[unstable(feature = "step_trait", issue = "42168")]
            impl Step for $i_wider {
                step_identical_methods!();
                step_signed_methods!($u_wider);

                #[inline]
                fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
                    if *start <= *end {
                        match end.checked_sub(*start) {
                            Some(result) => {
                                if let Ok(steps) = usize::try_from(result) {
                                    (steps, Some(steps))
                                } else {
                                    (usize::MAX, None)
                                }
                            }
                            // 如果差值大到例如 i128 都放不下，那么位数更少的 usize
                            // 也一定放不下。
                            None => (usize::MAX, None),
                        }
                    } else {
                        (0, None)
                    }
                }

                #[inline]
                fn forward_checked(start: Self, n: usize) -> Option<Self> {
                    start.checked_add(n as Self)
                }

                #[inline]
                fn backward_checked(start: Self, n: usize) -> Option<Self> {
                    start.checked_sub(n as Self)
                }
            }
        )+
    };
}

#[cfg(target_pointer_width = "64")]
step_integer_impls! {
    narrower than or same width as usize: [u8 i8], [u16 i16], [u32 i32], [u64 i64], [usize isize];
    wider than usize: [u128 i128];
}

#[cfg(target_pointer_width = "32")]
step_integer_impls! {
    narrower than or same width as usize: [u8 i8], [u16 i16], [u32 i32], [usize isize];
    wider than usize: [u64 i64], [u128 i128];
}

#[cfg(target_pointer_width = "16")]
step_integer_impls! {
    narrower than or same width as usize: [u8 i8], [u16 i16], [usize isize];
    wider than usize: [u32 i32], [u64 i64], [u128 i128];
}

#[unstable(feature = "step_trait", issue = "42168")]
impl Step for char {
    #[inline]
    fn steps_between(&start: &char, &end: &char) -> (usize, Option<usize>) {
        let start = start as u32;
        let end = end as u32;
        if start <= end {
            let count = end - start;
            if start < 0xD800 && 0xE000 <= end {
                if let Ok(steps) = usize::try_from(count - 0x800) {
                    (steps, Some(steps))
                } else {
                    (usize::MAX, None)
                }
            } else {
                if let Ok(steps) = usize::try_from(count) {
                    (steps, Some(steps))
                } else {
                    (usize::MAX, None)
                }
            }
        } else {
            (0, None)
        }
    }

    #[inline]
    fn forward_checked(start: char, count: usize) -> Option<char> {
        let start = start as u32;
        let mut res = Step::forward_checked(start, count)?;
        if start < 0xD800 && 0xD800 <= res {
            res = Step::forward_checked(res, 0x800)?;
        }
        if res <= char::MAX as u32 {
            // SAFETY: res 是有效 Unicode 标量值
            // (小于 0x110000 且不在 0xD800..0xE000 内)
            Some(unsafe { char::from_u32_unchecked(res) })
        } else {
            None
        }
    }

    #[inline]
    fn backward_checked(start: char, count: usize) -> Option<char> {
        let start = start as u32;
        let mut res = Step::backward_checked(start, count)?;
        if start >= 0xE000 && 0xE000 > res {
            res = Step::backward_checked(res, 0x800)?;
        }
        // SAFETY: res 是有效 Unicode 标量值
        // (小于 0x110000 且不在 0xD800..0xE000 内)。
        Some(unsafe { char::from_u32_unchecked(res) })
    }

    #[inline]
    unsafe fn forward_unchecked(start: char, count: usize) -> char {
        let start = start as u32;
        // SAFETY: 调用方必须保证这不会溢出 char 的取值范围。
        let mut res = unsafe { Step::forward_unchecked(start, count) };
        if start < 0xD800 && 0xD800 <= res {
            // SAFETY: 调用方必须保证这不会溢出 char 的取值范围。
            res = unsafe { Step::forward_unchecked(res, 0x800) };
        }
        // SAFETY: 根据前面的契约，调用方保证结果是有效 char。
        unsafe { char::from_u32_unchecked(res) }
    }

    #[inline]
    unsafe fn backward_unchecked(start: char, count: usize) -> char {
        let start = start as u32;
        // SAFETY: 调用方必须保证这不会溢出 char 的取值范围。
        let mut res = unsafe { Step::backward_unchecked(start, count) };
        if start >= 0xE000 && 0xE000 > res {
            // SAFETY: 调用方必须保证这不会溢出 char 的取值范围。
            res = unsafe { Step::backward_unchecked(res, 0x800) };
        }
        // SAFETY: 根据前面的契约，调用方保证结果是有效 char。
        unsafe { char::from_u32_unchecked(res) }
    }
}

#[unstable(feature = "step_trait", issue = "42168")]
impl Step for AsciiChar {
    #[inline]
    fn steps_between(&start: &AsciiChar, &end: &AsciiChar) -> (usize, Option<usize>) {
        Step::steps_between(&start.to_u8(), &end.to_u8())
    }

    #[inline]
    fn forward_checked(start: AsciiChar, count: usize) -> Option<AsciiChar> {
        let end = Step::forward_checked(start.to_u8(), count)?;
        AsciiChar::from_u8(end)
    }

    #[inline]
    fn backward_checked(start: AsciiChar, count: usize) -> Option<AsciiChar> {
        let end = Step::backward_checked(start.to_u8(), count)?;

        // SAFETY: 低于有效 ASCII 字符上界的值同样是有效 ASCII。
        Some(unsafe { AsciiChar::from_u8_unchecked(end) })
    }

    #[inline]
    unsafe fn forward_unchecked(start: AsciiChar, count: usize) -> AsciiChar {
        // SAFETY: 调用者断言结果是有效 ASCII 字符，因此它也是有效 u8。
        let end = unsafe { Step::forward_unchecked(start.to_u8(), count) };

        // SAFETY: 调用者断言结果是有效 ASCII 字符。
        unsafe { AsciiChar::from_u8_unchecked(end) }
    }

    #[inline]
    unsafe fn backward_unchecked(start: AsciiChar, count: usize) -> AsciiChar {
        // SAFETY: 调用者断言结果是有效 ASCII 字符，因此它也是有效 u8。
        let end = unsafe { Step::backward_unchecked(start.to_u8(), count) };

        // SAFETY: 调用者断言结果是有效 ASCII 字符。
        unsafe { AsciiChar::from_u8_unchecked(end) }
    }
}

#[unstable(feature = "step_trait", issue = "42168")]
impl Step for Ipv4Addr {
    #[inline]
    fn steps_between(&start: &Ipv4Addr, &end: &Ipv4Addr) -> (usize, Option<usize>) {
        u32::steps_between(&start.to_bits(), &end.to_bits())
    }

    #[inline]
    fn forward_checked(start: Ipv4Addr, count: usize) -> Option<Ipv4Addr> {
        u32::forward_checked(start.to_bits(), count).map(Ipv4Addr::from_bits)
    }

    #[inline]
    fn backward_checked(start: Ipv4Addr, count: usize) -> Option<Ipv4Addr> {
        u32::backward_checked(start.to_bits(), count).map(Ipv4Addr::from_bits)
    }

    #[inline]
    unsafe fn forward_unchecked(start: Ipv4Addr, count: usize) -> Ipv4Addr {
        // SAFETY: 由于 u32 和 Ipv4Addr 可无损互相转换，
        //   这里与 u32 版本同样安全。
        Ipv4Addr::from_bits(unsafe { u32::forward_unchecked(start.to_bits(), count) })
    }

    #[inline]
    unsafe fn backward_unchecked(start: Ipv4Addr, count: usize) -> Ipv4Addr {
        // SAFETY: 由于 u32 和 Ipv4Addr 可无损互相转换，
        //   这里与 u32 版本同样安全。
        Ipv4Addr::from_bits(unsafe { u32::backward_unchecked(start.to_bits(), count) })
    }
}

#[unstable(feature = "step_trait", issue = "42168")]
impl Step for Ipv6Addr {
    #[inline]
    fn steps_between(&start: &Ipv6Addr, &end: &Ipv6Addr) -> (usize, Option<usize>) {
        u128::steps_between(&start.to_bits(), &end.to_bits())
    }

    #[inline]
    fn forward_checked(start: Ipv6Addr, count: usize) -> Option<Ipv6Addr> {
        u128::forward_checked(start.to_bits(), count).map(Ipv6Addr::from_bits)
    }

    #[inline]
    fn backward_checked(start: Ipv6Addr, count: usize) -> Option<Ipv6Addr> {
        u128::backward_checked(start.to_bits(), count).map(Ipv6Addr::from_bits)
    }

    #[inline]
    unsafe fn forward_unchecked(start: Ipv6Addr, count: usize) -> Ipv6Addr {
        // SAFETY: 由于 u128 和 Ipv6Addr 可无损互相转换，
        //   这里与 u128 版本同样安全。
        Ipv6Addr::from_bits(unsafe { u128::forward_unchecked(start.to_bits(), count) })
    }

    #[inline]
    unsafe fn backward_unchecked(start: Ipv6Addr, count: usize) -> Ipv6Addr {
        // SAFETY: 由于 u128 和 Ipv6Addr 可无损互相转换，
        //   这里与 u128 版本同样安全。
        Ipv6Addr::from_bits(unsafe { u128::backward_unchecked(start.to_bits(), count) })
    }
}

macro_rules! range_exact_iter_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        impl ExactSizeIterator for ops::Range<$t> { }
    )*)
}

/// 安全性: 该宏只能用于 `Copy` 类型，并且生成的 range 必须具有精确 `size_hint()`，
/// 其中上界不得为 `None`。
macro_rules! unsafe_range_trusted_random_access_impl {
    ($($t:ty)*) => ($(
        #[doc(hidden)]
        #[unstable(feature = "trusted_random_access", issue = "none")]
        unsafe impl TrustedRandomAccess for ops::Range<$t> {}

        #[doc(hidden)]
        #[unstable(feature = "trusted_random_access", issue = "none")]
        unsafe impl TrustedRandomAccessNoCoerce for ops::Range<$t> {
            const MAY_HAVE_SIDE_EFFECT: bool = false;
        }
    )*)
}

macro_rules! range_incl_exact_iter_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "inclusive_range", since = "1.26.0")]
        impl ExactSizeIterator for ops::RangeInclusive<$t> { }
    )*)
}

/// `Range` 的 specialization 实现。
trait RangeIteratorImpl {
    type Item;

    // Iterator 相关方法
    fn spec_next(&mut self) -> Option<Self::Item>;
    fn spec_nth(&mut self, n: usize) -> Option<Self::Item>;
    fn spec_advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>>;

    // DoubleEndedIterator 相关方法
    fn spec_next_back(&mut self) -> Option<Self::Item>;
    fn spec_nth_back(&mut self, n: usize) -> Option<Self::Item>;
    fn spec_advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>>;
}

impl<A: Step> RangeIteratorImpl for ops::Range<A> {
    type Item = A;

    #[inline]
    default fn spec_next(&mut self) -> Option<A> {
        if self.start < self.end {
            let n =
                Step::forward_checked(self.start.clone(), 1).expect("`Step` invariants not upheld");
            Some(mem::replace(&mut self.start, n))
        } else {
            None
        }
    }

    #[inline]
    default fn spec_nth(&mut self, n: usize) -> Option<A> {
        if let Some(plus_n) = Step::forward_checked(self.start.clone(), n) {
            if plus_n < self.end {
                self.start =
                    Step::forward_checked(plus_n.clone(), 1).expect("`Step` invariants not upheld");
                return Some(plus_n);
            }
        }

        self.start = self.end.clone();
        None
    }

    #[inline]
    default fn spec_advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        let steps = Step::steps_between(&self.start, &self.end);
        let available = steps.1.unwrap_or(steps.0);

        let taken = available.min(n);

        self.start =
            Step::forward_checked(self.start.clone(), taken).expect("`Step` invariants not upheld");

        NonZero::new(n - taken).map_or(Ok(()), Err)
    }

    #[inline]
    default fn spec_next_back(&mut self) -> Option<A> {
        if self.start < self.end {
            self.end =
                Step::backward_checked(self.end.clone(), 1).expect("`Step` invariants not upheld");
            Some(self.end.clone())
        } else {
            None
        }
    }

    #[inline]
    default fn spec_nth_back(&mut self, n: usize) -> Option<A> {
        if let Some(minus_n) = Step::backward_checked(self.end.clone(), n) {
            if minus_n > self.start {
                self.end =
                    Step::backward_checked(minus_n, 1).expect("`Step` invariants not upheld");
                return Some(self.end.clone());
            }
        }

        self.end = self.start.clone();
        None
    }

    #[inline]
    default fn spec_advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        let steps = Step::steps_between(&self.start, &self.end);
        let available = steps.1.unwrap_or(steps.0);

        let taken = available.min(n);

        self.end =
            Step::backward_checked(self.end.clone(), taken).expect("`Step` invariants not upheld");

        NonZero::new(n - taken).map_or(Ok(()), Err)
    }
}

impl<T: TrustedStep> RangeIteratorImpl for ops::Range<T> {
    #[inline]
    fn spec_next(&mut self) -> Option<T> {
        if self.start < self.end {
            let old = self.start;
            // SAFETY: 刚刚检查了前置条件。
            self.start = unsafe { Step::forward_unchecked(old, 1) };
            Some(old)
        } else {
            None
        }
    }

    #[inline]
    fn spec_nth(&mut self, n: usize) -> Option<T> {
        if let Some(plus_n) = Step::forward_checked(self.start, n) {
            if plus_n < self.end {
                // SAFETY: 刚刚检查了前置条件。
                self.start = unsafe { Step::forward_unchecked(plus_n, 1) };
                return Some(plus_n);
            }
        }

        self.start = self.end;
        None
    }

    #[inline]
    fn spec_advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        let steps = Step::steps_between(&self.start, &self.end);
        let available = steps.1.unwrap_or(steps.0);

        let taken = available.min(n);

        // SAFETY: 上述条件保证 count 在边界内。如果 start <= end，则 steps_between
        // 要么返回一个可用于截断的边界，要么返回 None；后者结合初始不等式表示步数
        // 超过 usize::MAX。否则返回 0，而 0 始终可安全使用。
        self.start = unsafe { Step::forward_unchecked(self.start, taken) };

        NonZero::new(n - taken).map_or(Ok(()), Err)
    }

    #[inline]
    fn spec_next_back(&mut self) -> Option<T> {
        if self.start < self.end {
            // SAFETY: 刚刚检查了前置条件。
            self.end = unsafe { Step::backward_unchecked(self.end, 1) };
            Some(self.end)
        } else {
            None
        }
    }

    #[inline]
    fn spec_nth_back(&mut self, n: usize) -> Option<T> {
        if let Some(minus_n) = Step::backward_checked(self.end, n) {
            if minus_n > self.start {
                // SAFETY: 刚刚检查了前置条件。
                self.end = unsafe { Step::backward_unchecked(minus_n, 1) };
                return Some(self.end);
            }
        }

        self.end = self.start;
        None
    }

    #[inline]
    fn spec_advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        let steps = Step::steps_between(&self.start, &self.end);
        let available = steps.1.unwrap_or(steps.0);

        let taken = available.min(n);

        // SAFETY: 与 spec_advance_by() 实现的理由相同。
        self.end = unsafe { Step::backward_unchecked(self.end, taken) };

        NonZero::new(n - taken).map_or(Ok(()), Err)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A: Step> Iterator for ops::Range<A> {
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        self.spec_next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.start < self.end {
            Step::steps_between(&self.start, &self.end)
        } else {
            (0, Some(0))
        }
    }

    #[inline]
    fn count(self) -> usize {
        if self.start < self.end {
            Step::steps_between(&self.start, &self.end).1.expect("count overflowed usize")
        } else {
            0
        }
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<A> {
        self.spec_nth(n)
    }

    #[inline]
    fn last(mut self) -> Option<A> {
        self.next_back()
    }

    #[inline]
    fn min(mut self) -> Option<A>
    where
        A: Ord,
    {
        self.next()
    }

    #[inline]
    fn max(mut self) -> Option<A>
    where
        A: Ord,
    {
        self.next_back()
    }

    #[inline]
    fn is_sorted(self) -> bool {
        true
    }

    #[inline]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.spec_advance_by(n)
    }

    #[inline]
    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item
    where
        Self: TrustedRandomAccessNoCoerce,
    {
        // SAFETY: TrustedRandomAccess 契约要求调用方只传入边界内索引。
        // 此外，Self: TrustedRandomAccess 只为 Copy 类型实现，因此即使重复读取同一索引
        // 也是安全的。
        unsafe { Step::forward_unchecked(self.start.clone(), idx) }
    }
}

// 这些宏为多种 range 类型生成 `ExactSizeIterator` 实现。
//
// * `ExactSizeIterator::len` 要求始终返回精确的 `usize`，
//   因此任何 range 都不能长于 `usize::MAX`。
// * 对 `Range<_>` 中的整数类型，窄于或等宽于 `usize` 的类型满足这一点。
//   对 `RangeInclusive<_>` 中的整数类型，只有*严格窄于* `usize` 的类型满足这一点，
//   因为例如 `(0..=u64::MAX).len()` 会是 `u64::MAX + 1`。
range_exact_iter_impl! {
    usize u8 u16
    isize i8 i16

    // 按照上面的推理，这些实现并不正确；但它们已在 Rust 1.0.0 稳定，移除会造成
    // 破坏性变更。因此例如 `(0..66_000_u32).len()` 在 16 位平台上仍会无错误、无警告
    // 地编译，但继续给出错误结果。
    u32
    i32
}

unsafe_range_trusted_random_access_impl! {
    usize u8 u16
    isize i8 i16
}

#[cfg(target_pointer_width = "32")]
unsafe_range_trusted_random_access_impl! {
    u32 i32
}

#[cfg(target_pointer_width = "64")]
unsafe_range_trusted_random_access_impl! {
    u32 i32
    u64 i64
}

range_incl_exact_iter_impl! {
    u8
    i8

    // 按照上面的推理，这些实现并不正确；但它们已在 Rust 1.26.0 稳定，移除会造成
    // 破坏性变更。因此例如 `(0..=u16::MAX).len()` 在 16 位平台上仍会无错误、无警告
    // 地编译，但继续给出错误结果。
    u16
    i16
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A: Step> DoubleEndedIterator for ops::Range<A> {
    #[inline]
    fn next_back(&mut self) -> Option<A> {
        self.spec_next_back()
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<A> {
        self.spec_nth_back(n)
    }

    #[inline]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.spec_advance_back_by(n)
    }
}

// SAFETY:
// `Step::steps_between` 具有以下不变量:
//
// > * `steps_between(&a, &b) == (n, Some(n))` 仅当 `a <= b`
// >   * 注意，`a <= b` 并不推出 `steps_between(&a, &b) != (n, None)`；
// >     如果到达 `b` 需要超过 `usize::MAX` 步，就会出现这种情况。
// > * 如果 `a > b`，则 `steps_between(&a, &b) == (0, None)`
//
// 第一个不变量是 `TrustedLen` sound 通常所需的条件。附加说明满足了 `TrustedLen`
// 的另一条不变量。
//
// > 只有当实际迭代器长度大于 `usize::MAX` 时，上界才允许为 `None`。
//
// 只要 `PartialOrd` 实现正确，第二个不变量在逻辑上可由第一个推出；无论如何这里都
// 明确写出。如果 `a < b`，`ops::Range<A: Step>::size_hint` 会返回 `(0, Some(0))`。
// 因此第二个不变量也被维护。
#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A: TrustedStep> TrustedLen for ops::Range<A> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<A: Step> FusedIterator for ops::Range<A> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A: Step> Iterator for ops::RangeFrom<A> {
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        let n = Step::forward(self.start.clone(), 1);
        Some(mem::replace(&mut self.start, n))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<A> {
        let plus_n = Step::forward(self.start.clone(), n);
        self.start = Step::forward(plus_n.clone(), 1);
        Some(plus_n)
    }
}

// SAFETY: 见上面对 `ops::Range<A>` 的实现。
#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A: TrustedStep> TrustedLen for ops::RangeFrom<A> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<A: Step> FusedIterator for ops::RangeFrom<A> {}

trait RangeInclusiveIteratorImpl {
    type Item;

    // Iterator 相关方法
    fn spec_next(&mut self) -> Option<Self::Item>;
    fn spec_try_fold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>;

    // DoubleEndedIterator 相关方法
    fn spec_next_back(&mut self) -> Option<Self::Item>;
    fn spec_try_rfold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>;
}

impl<A: Step> RangeInclusiveIteratorImpl for ops::RangeInclusive<A> {
    type Item = A;

    #[inline]
    default fn spec_next(&mut self) -> Option<A> {
        if self.is_empty() {
            return None;
        }
        let is_iterating = self.start < self.end;
        Some(if is_iterating {
            let n =
                Step::forward_checked(self.start.clone(), 1).expect("`Step` invariants not upheld");
            mem::replace(&mut self.start, n)
        } else {
            self.exhausted = true;
            self.start.clone()
        })
    }

    #[inline]
    default fn spec_try_fold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, A) -> R,
        R: Try<Output = B>,
    {
        if self.is_empty() {
            return try { init };
        }

        let mut accum = init;

        while self.start < self.end {
            let n =
                Step::forward_checked(self.start.clone(), 1).expect("`Step` invariants not upheld");
            let n = mem::replace(&mut self.start, n);
            accum = f(accum, n)?;
        }

        self.exhausted = true;

        if self.start == self.end {
            accum = f(accum, self.start.clone())?;
        }

        try { accum }
    }

    #[inline]
    default fn spec_next_back(&mut self) -> Option<A> {
        if self.is_empty() {
            return None;
        }
        let is_iterating = self.start < self.end;
        Some(if is_iterating {
            let n =
                Step::backward_checked(self.end.clone(), 1).expect("`Step` invariants not upheld");
            mem::replace(&mut self.end, n)
        } else {
            self.exhausted = true;
            self.end.clone()
        })
    }

    #[inline]
    default fn spec_try_rfold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, A) -> R,
        R: Try<Output = B>,
    {
        if self.is_empty() {
            return try { init };
        }

        let mut accum = init;

        while self.start < self.end {
            let n =
                Step::backward_checked(self.end.clone(), 1).expect("`Step` invariants not upheld");
            let n = mem::replace(&mut self.end, n);
            accum = f(accum, n)?;
        }

        self.exhausted = true;

        if self.start == self.end {
            accum = f(accum, self.start.clone())?;
        }

        try { accum }
    }
}

impl<T: TrustedStep> RangeInclusiveIteratorImpl for ops::RangeInclusive<T> {
    #[inline]
    fn spec_next(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let is_iterating = self.start < self.end;
        Some(if is_iterating {
            // SAFETY: 刚刚检查了前置条件。
            let n = unsafe { Step::forward_unchecked(self.start, 1) };
            mem::replace(&mut self.start, n)
        } else {
            self.exhausted = true;
            self.start
        })
    }

    #[inline]
    fn spec_try_fold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, T) -> R,
        R: Try<Output = B>,
    {
        if self.is_empty() {
            return try { init };
        }

        let mut accum = init;

        while self.start < self.end {
            // SAFETY: 刚刚检查了前置条件。
            let n = unsafe { Step::forward_unchecked(self.start, 1) };
            let n = mem::replace(&mut self.start, n);
            accum = f(accum, n)?;
        }

        self.exhausted = true;

        if self.start == self.end {
            accum = f(accum, self.start)?;
        }

        try { accum }
    }

    #[inline]
    fn spec_next_back(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let is_iterating = self.start < self.end;
        Some(if is_iterating {
            // SAFETY: 刚刚检查了前置条件。
            let n = unsafe { Step::backward_unchecked(self.end, 1) };
            mem::replace(&mut self.end, n)
        } else {
            self.exhausted = true;
            self.end
        })
    }

    #[inline]
    fn spec_try_rfold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, T) -> R,
        R: Try<Output = B>,
    {
        if self.is_empty() {
            return try { init };
        }

        let mut accum = init;

        while self.start < self.end {
            // SAFETY: 刚刚检查了前置条件。
            let n = unsafe { Step::backward_unchecked(self.end, 1) };
            let n = mem::replace(&mut self.end, n);
            accum = f(accum, n)?;
        }

        self.exhausted = true;

        if self.start == self.end {
            accum = f(accum, self.start)?;
        }

        try { accum }
    }
}

#[stable(feature = "inclusive_range", since = "1.26.0")]
impl<A: Step> Iterator for ops::RangeInclusive<A> {
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        self.spec_next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.is_empty() {
            return (0, Some(0));
        }

        let hint = Step::steps_between(&self.start, &self.end);
        (hint.0.saturating_add(1), hint.1.and_then(|steps| steps.checked_add(1)))
    }

    #[inline]
    fn count(self) -> usize {
        if self.is_empty() {
            return 0;
        }

        Step::steps_between(&self.start, &self.end)
            .1
            .and_then(|steps| steps.checked_add(1))
            .expect("count overflowed usize")
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<A> {
        if self.is_empty() {
            return None;
        }

        if let Some(plus_n) = Step::forward_checked(self.start.clone(), n) {
            use crate::cmp::Ordering::*;

            match plus_n.partial_cmp(&self.end) {
                Some(Less) => {
                    self.start = Step::forward(plus_n.clone(), 1);
                    return Some(plus_n);
                }
                Some(Equal) => {
                    self.start = plus_n.clone();
                    self.exhausted = true;
                    return Some(plus_n);
                }
                _ => {}
            }
        }

        self.start = self.end.clone();
        self.exhausted = true;
        None
    }

    #[inline]
    fn try_fold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        self.spec_try_fold(init, f)
    }

    impl_fold_via_try_fold! { fold -> try_fold }

    #[inline]
    fn last(mut self) -> Option<A> {
        self.next_back()
    }

    #[inline]
    fn min(mut self) -> Option<A>
    where
        A: Ord,
    {
        self.next()
    }

    #[inline]
    fn max(mut self) -> Option<A>
    where
        A: Ord,
    {
        self.next_back()
    }

    #[inline]
    fn is_sorted(self) -> bool {
        true
    }
}

#[stable(feature = "inclusive_range", since = "1.26.0")]
impl<A: Step> DoubleEndedIterator for ops::RangeInclusive<A> {
    #[inline]
    fn next_back(&mut self) -> Option<A> {
        self.spec_next_back()
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<A> {
        if self.is_empty() {
            return None;
        }

        if let Some(minus_n) = Step::backward_checked(self.end.clone(), n) {
            use crate::cmp::Ordering::*;

            match minus_n.partial_cmp(&self.start) {
                Some(Greater) => {
                    self.end = Step::backward(minus_n.clone(), 1);
                    return Some(minus_n);
                }
                Some(Equal) => {
                    self.end = minus_n.clone();
                    self.exhausted = true;
                    return Some(minus_n);
                }
                _ => {}
            }
        }

        self.end = self.start.clone();
        self.exhausted = true;
        None
    }

    #[inline]
    fn try_rfold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        self.spec_try_rfold(init, f)
    }

    impl_fold_via_try_fold! { rfold -> try_rfold }
}

// SAFETY: 见上面对 `ops::Range<A>` 的实现。
#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A: TrustedStep> TrustedLen for ops::RangeInclusive<A> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<A: Step> FusedIterator for ops::RangeInclusive<A> {}
