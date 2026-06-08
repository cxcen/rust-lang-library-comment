//! 内建数值类型的数值 trait 和函数。

#![stable(feature = "rust1", since = "1.0.0")]

use crate::panic::const_panic;
use crate::str::FromStr;
use crate::ub_checks::assert_unsafe_precondition;
use crate::{ascii, intrinsics, mem};

// FIXME(const-hack): 用于绕开 const 上下文中不允许使用 `?` 运算符的问题。
macro_rules! try_opt {
    ($e:expr) => {
        match $e {
            Some(x) => x,
            None => return None,
        }
    };
}

// 当生成代码需要在有符号和无符号类型之间不同时使用。
macro_rules! sign_dependent_expr {
    (signed ? if signed { $signed_case:expr } if unsigned { $unsigned_case:expr } ) => {
        $signed_case
    };
    (unsigned ? if signed { $signed_case:expr } if unsigned { $unsigned_case:expr } ) => {
        $unsigned_case
    };
}

// 这些模块严格说都是私有的，只是为了 coretests 而暴露：
#[cfg(not(no_fp_fmt_parse))]
pub mod bignum;
#[cfg(not(no_fp_fmt_parse))]
pub mod dec2flt;
#[cfg(not(no_fp_fmt_parse))]
pub mod diy_float;
#[cfg(not(no_fp_fmt_parse))]
pub mod flt2dec;
pub mod fmt;

#[macro_use]
mod int_macros; // 导入 int_impl!
#[macro_use]
mod uint_macros; // 导入 uint_impl!

mod error;
mod int_bits;
mod int_log10;
mod int_sqrt;
pub(crate) mod libm;
mod nonzero;
mod overflow_panic;
mod saturating;
mod wrapping;

/// 100% 永久不稳定
#[doc(hidden)]
pub mod niche_types;

#[stable(feature = "rust1", since = "1.0.0")]
#[cfg(not(no_fp_fmt_parse))]
pub use dec2flt::ParseFloatError;
#[stable(feature = "int_error_matching", since = "1.55.0")]
pub use error::IntErrorKind;
#[stable(feature = "rust1", since = "1.0.0")]
pub use error::ParseIntError;
#[stable(feature = "try_from", since = "1.34.0")]
pub use error::TryFromIntError;
#[stable(feature = "generic_nonzero", since = "1.79.0")]
pub use nonzero::NonZero;
#[unstable(
    feature = "nonzero_internals",
    reason = "implementation detail which may disappear or be replaced at any time",
    issue = "none"
)]
pub use nonzero::ZeroablePrimitive;
#[stable(feature = "signed_nonzero", since = "1.34.0")]
pub use nonzero::{NonZeroI8, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI128, NonZeroIsize};
#[stable(feature = "nonzero", since = "1.28.0")]
pub use nonzero::{NonZeroU8, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU128, NonZeroUsize};
#[stable(feature = "saturating_int_impl", since = "1.74.0")]
pub use saturating::Saturating;
#[stable(feature = "rust1", since = "1.0.0")]
pub use wrapping::Wrapping;

macro_rules! u8_xe_bytes_doc {
    () => {
        "

**注意**：此函数对 `u8` 没有意义。字节序这个概念不适用于字节大小的整数。
提供此函数只是为了与更大的整数类型保持对称。

"
    };
}

macro_rules! i8_xe_bytes_doc {
    () => {
        "

**注意**：此函数对 `i8` 没有意义。字节序这个概念不适用于字节大小的整数。
提供此函数只是为了与更大的整数类型保持对称。你可以使用
[`cast_signed`](u8::cast_signed) 和 [`cast_unsigned`](Self::cast_unsigned)
在该类型与 `u8` 之间转换。

"
    };
}

macro_rules! usize_isize_to_xe_bytes_doc {
    () => {
        "

**注意**：此函数根据目标指针大小，返回长度为 2、4 或 8 字节的数组。

"
    };
}

macro_rules! usize_isize_from_xe_bytes_doc {
    () => {
        "

**注意**：此函数根据目标指针大小，接受长度为 2、4 或 8 字节的数组。

"
    };
}

macro_rules! midpoint_impl {
    ($SelfT:ty, unsigned) => {
        /// 计算 `self` 与 `rhs` 之间的中点（平均值）。
        ///
        /// `midpoint(a, b)` 就像在足够大的无符号整数类型中执行 `(a + b) / 2`
        /// 一样。这意味着结果始终向零舍入，并且永远不会发生溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".midpoint(4), 2);")]
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".midpoint(4), 2);")]
        /// ```
        #[stable(feature = "num_midpoint", since = "1.85.0")]
        #[rustc_const_stable(feature = "num_midpoint", since = "1.85.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[doc(alias = "average_floor")]
        #[doc(alias = "average")]
        #[inline]
        pub const fn midpoint(self, rhs: $SelfT) -> $SelfT {
            // 使用 Hacker's Delight 中众所周知的无分支算法，在不溢出的情况下计算
            // `(a + b) / 2`：`((a ^ b) >> 1) + (a & b)`。
            ((self ^ rhs) >> 1) + (self & rhs)
        }
    };
    ($SelfT:ty, signed) => {
        /// 计算 `self` 与 `rhs` 之间的中点（平均值）。
        ///
        /// `midpoint(a, b)` 就像在足够大的有符号整数类型中执行 `(a + b) / 2`
        /// 一样。这意味着结果始终向零舍入，并且永远不会发生溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".midpoint(4), 2);")]
        #[doc = concat!("assert_eq!((-1", stringify!($SelfT), ").midpoint(2), 0);")]
        #[doc = concat!("assert_eq!((-7", stringify!($SelfT), ").midpoint(0), -3);")]
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".midpoint(-7), -3);")]
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".midpoint(7), 3);")]
        /// ```
        #[stable(feature = "num_midpoint_signed", since = "1.87.0")]
        #[rustc_const_stable(feature = "num_midpoint_signed", since = "1.87.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[doc(alias = "average_floor")]
        #[doc(alias = "average_ceil")]
        #[doc(alias = "average")]
        #[inline]
        pub const fn midpoint(self, rhs: Self) -> Self {
            // 使用 Hacker's Delight 中众所周知的无分支算法，在不溢出的情况下计算
            // `(a + b) / 2`：`((a ^ b) >> 1) + (a & b)`。
            let t = ((self ^ rhs) >> 1) + (self & rhs);
            // 不过，当两个整数之和是负奇数时它会失效，因为其向下取整值比平均值小 1。
            // 因此这里调整结果。
            t + (if t < 0 { 1 } else { 0 } & (self ^ rhs))
        }
    };
    ($SelfT:ty, $WideT:ty, unsigned) => {
        /// 计算 `self` 与 `rhs` 之间的中点（平均值）。
        ///
        /// `midpoint(a, b)` 就像在足够大的无符号整数类型中执行 `(a + b) / 2`
        /// 一样。这意味着结果始终向零舍入，并且永远不会发生溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".midpoint(4), 2);")]
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".midpoint(4), 2);")]
        /// ```
        #[stable(feature = "num_midpoint", since = "1.85.0")]
        #[rustc_const_stable(feature = "num_midpoint", since = "1.85.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[doc(alias = "average_floor")]
        #[doc(alias = "average")]
        #[inline]
        pub const fn midpoint(self, rhs: $SelfT) -> $SelfT {
            ((self as $WideT + rhs as $WideT) / 2) as $SelfT
        }
    };
    ($SelfT:ty, $WideT:ty, signed) => {
        /// 计算 `self` 与 `rhs` 之间的中点（平均值）。
        ///
        /// `midpoint(a, b)` 就像在足够大的有符号整数类型中执行 `(a + b) / 2`
        /// 一样。这意味着结果始终向零舍入，并且永远不会发生溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".midpoint(4), 2);")]
        #[doc = concat!("assert_eq!((-1", stringify!($SelfT), ").midpoint(2), 0);")]
        #[doc = concat!("assert_eq!((-7", stringify!($SelfT), ").midpoint(0), -3);")]
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".midpoint(-7), -3);")]
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".midpoint(7), 3);")]
        /// ```
        #[stable(feature = "num_midpoint_signed", since = "1.87.0")]
        #[rustc_const_stable(feature = "num_midpoint_signed", since = "1.87.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[doc(alias = "average_floor")]
        #[doc(alias = "average_ceil")]
        #[doc(alias = "average")]
        #[inline]
        pub const fn midpoint(self, rhs: $SelfT) -> $SelfT {
            ((self as $WideT + rhs as $WideT) / 2) as $SelfT
        }
    };
}

impl i8 {
    int_impl! {
        Self = i8,
        ActualT = i8,
        UnsignedT = u8,
        BITS = 8,
        BITS_MINUS_ONE = 7,
        Min = -128,
        Max = 127,
        rot = 2,
        rot_op = "-0x7e",
        rot_result = "0xa",
        swap_op = "0x12",
        swapped = "0x12",
        reversed = "0x48",
        le_bytes = "[0x12]",
        be_bytes = "[0x12]",
        to_xe_bytes_doc = i8_xe_bytes_doc!(),
        from_xe_bytes_doc = i8_xe_bytes_doc!(),
        bound_condition = "",
    }
    midpoint_impl! { i8, i16, signed }
}

impl i16 {
    int_impl! {
        Self = i16,
        ActualT = i16,
        UnsignedT = u16,
        BITS = 16,
        BITS_MINUS_ONE = 15,
        Min = -32768,
        Max = 32767,
        rot = 4,
        rot_op = "-0x5ffd",
        rot_result = "0x3a",
        swap_op = "0x1234",
        swapped = "0x3412",
        reversed = "0x2c48",
        le_bytes = "[0x34, 0x12]",
        be_bytes = "[0x12, 0x34]",
        to_xe_bytes_doc = "",
        from_xe_bytes_doc = "",
        bound_condition = "",
    }
    midpoint_impl! { i16, i32, signed }
}

impl i32 {
    int_impl! {
        Self = i32,
        ActualT = i32,
        UnsignedT = u32,
        BITS = 32,
        BITS_MINUS_ONE = 31,
        Min = -2147483648,
        Max = 2147483647,
        rot = 8,
        rot_op = "0x10000b3",
        rot_result = "0xb301",
        swap_op = "0x12345678",
        swapped = "0x78563412",
        reversed = "0x1e6a2c48",
        le_bytes = "[0x78, 0x56, 0x34, 0x12]",
        be_bytes = "[0x12, 0x34, 0x56, 0x78]",
        to_xe_bytes_doc = "",
        from_xe_bytes_doc = "",
        bound_condition = "",
    }
    midpoint_impl! { i32, i64, signed }
}

impl i64 {
    int_impl! {
        Self = i64,
        ActualT = i64,
        UnsignedT = u64,
        BITS = 64,
        BITS_MINUS_ONE = 63,
        Min = -9223372036854775808,
        Max = 9223372036854775807,
        rot = 12,
        rot_op = "0xaa00000000006e1",
        rot_result = "0x6e10aa",
        swap_op = "0x1234567890123456",
        swapped = "0x5634129078563412",
        reversed = "0x6a2c48091e6a2c48",
        le_bytes = "[0x56, 0x34, 0x12, 0x90, 0x78, 0x56, 0x34, 0x12]",
        be_bytes = "[0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56]",
        to_xe_bytes_doc = "",
        from_xe_bytes_doc = "",
        bound_condition = "",
    }
    midpoint_impl! { i64, signed }
}

impl i128 {
    int_impl! {
        Self = i128,
        ActualT = i128,
        UnsignedT = u128,
        BITS = 128,
        BITS_MINUS_ONE = 127,
        Min = -170141183460469231731687303715884105728,
        Max = 170141183460469231731687303715884105727,
        rot = 16,
        rot_op = "0x13f40000000000000000000000004f76",
        rot_result = "0x4f7613f4",
        swap_op = "0x12345678901234567890123456789012",
        swapped = "0x12907856341290785634129078563412",
        reversed = "0x48091e6a2c48091e6a2c48091e6a2c48",
        le_bytes = "[0x12, 0x90, 0x78, 0x56, 0x34, 0x12, 0x90, 0x78, \
            0x56, 0x34, 0x12, 0x90, 0x78, 0x56, 0x34, 0x12]",
        be_bytes = "[0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56, \
            0x78, 0x90, 0x12, 0x34, 0x56, 0x78, 0x90, 0x12]",
        to_xe_bytes_doc = "",
        from_xe_bytes_doc = "",
        bound_condition = "",
    }
    midpoint_impl! { i128, signed }
}

#[cfg(target_pointer_width = "16")]
impl isize {
    int_impl! {
        Self = isize,
        ActualT = i16,
        UnsignedT = usize,
        BITS = 16,
        BITS_MINUS_ONE = 15,
        Min = -32768,
        Max = 32767,
        rot = 4,
        rot_op = "-0x5ffd",
        rot_result = "0x3a",
        swap_op = "0x1234",
        swapped = "0x3412",
        reversed = "0x2c48",
        le_bytes = "[0x34, 0x12]",
        be_bytes = "[0x12, 0x34]",
        to_xe_bytes_doc = usize_isize_to_xe_bytes_doc!(),
        from_xe_bytes_doc = usize_isize_from_xe_bytes_doc!(),
        bound_condition = " on 16-bit targets",
    }
    midpoint_impl! { isize, i32, signed }
}

#[cfg(target_pointer_width = "32")]
impl isize {
    int_impl! {
        Self = isize,
        ActualT = i32,
        UnsignedT = usize,
        BITS = 32,
        BITS_MINUS_ONE = 31,
        Min = -2147483648,
        Max = 2147483647,
        rot = 8,
        rot_op = "0x10000b3",
        rot_result = "0xb301",
        swap_op = "0x12345678",
        swapped = "0x78563412",
        reversed = "0x1e6a2c48",
        le_bytes = "[0x78, 0x56, 0x34, 0x12]",
        be_bytes = "[0x12, 0x34, 0x56, 0x78]",
        to_xe_bytes_doc = usize_isize_to_xe_bytes_doc!(),
        from_xe_bytes_doc = usize_isize_from_xe_bytes_doc!(),
        bound_condition = " on 32-bit targets",
    }
    midpoint_impl! { isize, i64, signed }
}

#[cfg(target_pointer_width = "64")]
impl isize {
    int_impl! {
        Self = isize,
        ActualT = i64,
        UnsignedT = usize,
        BITS = 64,
        BITS_MINUS_ONE = 63,
        Min = -9223372036854775808,
        Max = 9223372036854775807,
        rot = 12,
        rot_op = "0xaa00000000006e1",
        rot_result = "0x6e10aa",
        swap_op = "0x1234567890123456",
        swapped = "0x5634129078563412",
        reversed = "0x6a2c48091e6a2c48",
        le_bytes = "[0x56, 0x34, 0x12, 0x90, 0x78, 0x56, 0x34, 0x12]",
        be_bytes = "[0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56]",
        to_xe_bytes_doc = usize_isize_to_xe_bytes_doc!(),
        from_xe_bytes_doc = usize_isize_from_xe_bytes_doc!(),
        bound_condition = " on 64-bit targets",
    }
    midpoint_impl! { isize, signed }
}

/// 如果该掩码选中的位被设置，则 ASCII 字符是小写。
const ASCII_CASE_MASK: u8 = 0b0010_0000;

impl u8 {
    uint_impl! {
        Self = u8,
        ActualT = u8,
        SignedT = i8,
        BITS = 8,
        BITS_MINUS_ONE = 7,
        MAX = 255,
        rot = 2,
        rot_op = "0x82",
        rot_result = "0xa",
        fsh_op = "0x36",
        fshl_result = "0x8",
        fshr_result = "0x8d",
        swap_op = "0x12",
        swapped = "0x12",
        reversed = "0x48",
        le_bytes = "[0x12]",
        be_bytes = "[0x12]",
        to_xe_bytes_doc = u8_xe_bytes_doc!(),
        from_xe_bytes_doc = u8_xe_bytes_doc!(),
        bound_condition = "",
    }
    midpoint_impl! { u8, u16, unsigned }

    /// 检查该值是否在 ASCII 范围内。
    ///
    /// # 示例
    ///
    /// ```
    /// let ascii = 97u8;
    /// let non_ascii = 150u8;
    ///
    /// assert!(ascii.is_ascii());
    /// assert!(!non_ascii.is_ascii());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_u8_is_ascii", since = "1.43.0")]
    #[inline]
    pub const fn is_ascii(&self) -> bool {
        *self <= 127
    }

    /// 如果该字节的值在 ASCII 范围内，则将其作为
    /// [ASCII 字符](ascii::Char) 返回；否则返回 `None`。
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn as_ascii(&self) -> Option<ascii::Char> {
        ascii::Char::from_u8(*self)
    }

    /// 将该字节转换为 [ASCII 字符](ascii::Char)，不检查其是否有效。
    ///
    /// # 安全性(Safety）
    ///
    /// 该字节必须是有效 ASCII，否则属于 UB。
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const unsafe fn as_ascii_unchecked(&self) -> ascii::Char {
        assert_unsafe_precondition!(
            check_library_ub,
            "as_ascii_unchecked requires that the byte is valid ASCII",
            (it: &u8 = self) => it.is_ascii()
        );

        // SAFETY: 调用方承诺该字节是 ASCII。
        unsafe { ascii::Char::from_u8_unchecked(*self) }
    }

    /// 返回该值对应的 ASCII 大写副本。
    ///
    /// ASCII 字母 'a' 到 'z' 会映射到 'A' 到 'Z'，
    /// 但非 ASCII 字母保持不变。
    ///
    /// 若要就地转为大写，请使用 [`make_ascii_uppercase`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let lowercase_a = 97u8;
    ///
    /// assert_eq!(65, lowercase_a.to_ascii_uppercase());
    /// ```
    ///
    /// [`make_ascii_uppercase`]: Self::make_ascii_uppercase
    #[must_use = "to uppercase the value in-place, use `make_ascii_uppercase()`"]
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_ascii_methods_on_intrinsics", since = "1.52.0")]
    #[inline]
    pub const fn to_ascii_uppercase(&self) -> u8 {
        // 如果这是小写字母，则翻转第 6 位。
        *self ^ ((self.is_ascii_lowercase() as u8) * ASCII_CASE_MASK)
    }

    /// 返回该值对应的 ASCII 小写副本。
    ///
    /// ASCII 字母 'A' 到 'Z' 会映射到 'a' 到 'z'，
    /// 但非 ASCII 字母保持不变。
    ///
    /// 若要就地转为小写，请使用 [`make_ascii_lowercase`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = 65u8;
    ///
    /// assert_eq!(97, uppercase_a.to_ascii_lowercase());
    /// ```
    ///
    /// [`make_ascii_lowercase`]: Self::make_ascii_lowercase
    #[must_use = "to lowercase the value in-place, use `make_ascii_lowercase()`"]
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_ascii_methods_on_intrinsics", since = "1.52.0")]
    #[inline]
    pub const fn to_ascii_lowercase(&self) -> u8 {
        // 如果这是大写字母，则设置第 6 位。
        *self | (self.is_ascii_uppercase() as u8 * ASCII_CASE_MASK)
    }

    /// 假定 `self` 是 ASCII。
    #[inline]
    pub(crate) const fn ascii_change_case_unchecked(&self) -> u8 {
        *self ^ ASCII_CASE_MASK
    }

    /// 检查两个值按 ASCII 大小写不敏感规则是否匹配。
    ///
    /// 这等价于 `to_ascii_lowercase(a) == to_ascii_lowercase(b)`。
    ///
    /// # 示例
    ///
    /// ```
    /// let lowercase_a = 97u8;
    /// let uppercase_a = 65u8;
    ///
    /// assert!(lowercase_a.eq_ignore_ascii_case(&uppercase_a));
    /// ```
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_ascii_methods_on_intrinsics", since = "1.52.0")]
    #[inline]
    pub const fn eq_ignore_ascii_case(&self, other: &u8) -> bool {
        self.to_ascii_lowercase() == other.to_ascii_lowercase()
    }

    /// 将该值就地转换为对应的 ASCII 大写形式。
    ///
    /// ASCII 字母 'a' 到 'z' 会映射到 'A' 到 'Z'，
    /// 但非 ASCII 字母保持不变。
    ///
    /// 若要返回新的大写值而不修改现有值，请使用 [`to_ascii_uppercase`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut byte = b'a';
    ///
    /// byte.make_ascii_uppercase();
    ///
    /// assert_eq!(b'A', byte);
    /// ```
    ///
    /// [`to_ascii_uppercase`]: Self::to_ascii_uppercase
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_make_ascii", since = "1.84.0")]
    #[inline]
    pub const fn make_ascii_uppercase(&mut self) {
        *self = self.to_ascii_uppercase();
    }

    /// 将该值就地转换为对应的 ASCII 小写形式。
    ///
    /// ASCII 字母 'A' 到 'Z' 会映射到 'a' 到 'z'，
    /// 但非 ASCII 字母保持不变。
    ///
    /// 若要返回新的小写值而不修改现有值，请使用 [`to_ascii_lowercase`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut byte = b'A';
    ///
    /// byte.make_ascii_lowercase();
    ///
    /// assert_eq!(b'a', byte);
    /// ```
    ///
    /// [`to_ascii_lowercase`]: Self::to_ascii_lowercase
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_make_ascii", since = "1.84.0")]
    #[inline]
    pub const fn make_ascii_lowercase(&mut self) {
        *self = self.to_ascii_lowercase();
    }

    /// 检查该值是否为 ASCII 字母字符：
    ///
    /// - U+0041 'A' ..= U+005A 'Z'，或
    /// - U+0061 'a' ..= U+007A 'z'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = b'A';
    /// let uppercase_g = b'G';
    /// let a = b'a';
    /// let g = b'g';
    /// let zero = b'0';
    /// let percent = b'%';
    /// let space = b' ';
    /// let lf = b'\n';
    /// let esc = b'\x1b';
    ///
    /// assert!(uppercase_a.is_ascii_alphabetic());
    /// assert!(uppercase_g.is_ascii_alphabetic());
    /// assert!(a.is_ascii_alphabetic());
    /// assert!(g.is_ascii_alphabetic());
    /// assert!(!zero.is_ascii_alphabetic());
    /// assert!(!percent.is_ascii_alphabetic());
    /// assert!(!space.is_ascii_alphabetic());
    /// assert!(!lf.is_ascii_alphabetic());
    /// assert!(!esc.is_ascii_alphabetic());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_alphabetic(&self) -> bool {
        matches!(*self, b'A'..=b'Z' | b'a'..=b'z')
    }

    /// 检查该值是否为 ASCII 大写字符：
    /// U+0041 'A' ..= U+005A 'Z'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = b'A';
    /// let uppercase_g = b'G';
    /// let a = b'a';
    /// let g = b'g';
    /// let zero = b'0';
    /// let percent = b'%';
    /// let space = b' ';
    /// let lf = b'\n';
    /// let esc = b'\x1b';
    ///
    /// assert!(uppercase_a.is_ascii_uppercase());
    /// assert!(uppercase_g.is_ascii_uppercase());
    /// assert!(!a.is_ascii_uppercase());
    /// assert!(!g.is_ascii_uppercase());
    /// assert!(!zero.is_ascii_uppercase());
    /// assert!(!percent.is_ascii_uppercase());
    /// assert!(!space.is_ascii_uppercase());
    /// assert!(!lf.is_ascii_uppercase());
    /// assert!(!esc.is_ascii_uppercase());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_uppercase(&self) -> bool {
        matches!(*self, b'A'..=b'Z')
    }

    /// 检查该值是否为 ASCII 小写字符：
    /// U+0061 'a' ..= U+007A 'z'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = b'A';
    /// let uppercase_g = b'G';
    /// let a = b'a';
    /// let g = b'g';
    /// let zero = b'0';
    /// let percent = b'%';
    /// let space = b' ';
    /// let lf = b'\n';
    /// let esc = b'\x1b';
    ///
    /// assert!(!uppercase_a.is_ascii_lowercase());
    /// assert!(!uppercase_g.is_ascii_lowercase());
    /// assert!(a.is_ascii_lowercase());
    /// assert!(g.is_ascii_lowercase());
    /// assert!(!zero.is_ascii_lowercase());
    /// assert!(!percent.is_ascii_lowercase());
    /// assert!(!space.is_ascii_lowercase());
    /// assert!(!lf.is_ascii_lowercase());
    /// assert!(!esc.is_ascii_lowercase());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_lowercase(&self) -> bool {
        matches!(*self, b'a'..=b'z')
    }

    /// 检查该值是否为 ASCII 字母数字字符：
    ///
    /// - U+0041 'A' ..= U+005A 'Z'，或
    /// - U+0061 'a' ..= U+007A 'z'，或
    /// - U+0030 '0' ..= U+0039 '9'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = b'A';
    /// let uppercase_g = b'G';
    /// let a = b'a';
    /// let g = b'g';
    /// let zero = b'0';
    /// let percent = b'%';
    /// let space = b' ';
    /// let lf = b'\n';
    /// let esc = b'\x1b';
    ///
    /// assert!(uppercase_a.is_ascii_alphanumeric());
    /// assert!(uppercase_g.is_ascii_alphanumeric());
    /// assert!(a.is_ascii_alphanumeric());
    /// assert!(g.is_ascii_alphanumeric());
    /// assert!(zero.is_ascii_alphanumeric());
    /// assert!(!percent.is_ascii_alphanumeric());
    /// assert!(!space.is_ascii_alphanumeric());
    /// assert!(!lf.is_ascii_alphanumeric());
    /// assert!(!esc.is_ascii_alphanumeric());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_alphanumeric(&self) -> bool {
        matches!(*self, b'0'..=b'9') | matches!(*self, b'A'..=b'Z') | matches!(*self, b'a'..=b'z')
    }

    /// 检查该值是否为 ASCII 十进制数字：
    /// U+0030 '0' ..= U+0039 '9'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = b'A';
    /// let uppercase_g = b'G';
    /// let a = b'a';
    /// let g = b'g';
    /// let zero = b'0';
    /// let percent = b'%';
    /// let space = b' ';
    /// let lf = b'\n';
    /// let esc = b'\x1b';
    ///
    /// assert!(!uppercase_a.is_ascii_digit());
    /// assert!(!uppercase_g.is_ascii_digit());
    /// assert!(!a.is_ascii_digit());
    /// assert!(!g.is_ascii_digit());
    /// assert!(zero.is_ascii_digit());
    /// assert!(!percent.is_ascii_digit());
    /// assert!(!space.is_ascii_digit());
    /// assert!(!lf.is_ascii_digit());
    /// assert!(!esc.is_ascii_digit());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_digit(&self) -> bool {
        matches!(*self, b'0'..=b'9')
    }

    /// 检查该值是否为 ASCII 八进制数字：
    /// U+0030 '0' ..= U+0037 '7'.
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(is_ascii_octdigit)]
    ///
    /// let uppercase_a = b'A';
    /// let a = b'a';
    /// let zero = b'0';
    /// let seven = b'7';
    /// let nine = b'9';
    /// let percent = b'%';
    /// let lf = b'\n';
    ///
    /// assert!(!uppercase_a.is_ascii_octdigit());
    /// assert!(!a.is_ascii_octdigit());
    /// assert!(zero.is_ascii_octdigit());
    /// assert!(seven.is_ascii_octdigit());
    /// assert!(!nine.is_ascii_octdigit());
    /// assert!(!percent.is_ascii_octdigit());
    /// assert!(!lf.is_ascii_octdigit());
    /// ```
    #[must_use]
    #[unstable(feature = "is_ascii_octdigit", issue = "101288")]
    #[inline]
    pub const fn is_ascii_octdigit(&self) -> bool {
        matches!(*self, b'0'..=b'7')
    }

    /// 检查该值是否为 ASCII 十六进制数字：
    ///
    /// - U+0030 '0' ..= U+0039 '9'，或
    /// - U+0041 'A' ..= U+0046 'F'，或
    /// - U+0061 'a' ..= U+0066 'f'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = b'A';
    /// let uppercase_g = b'G';
    /// let a = b'a';
    /// let g = b'g';
    /// let zero = b'0';
    /// let percent = b'%';
    /// let space = b' ';
    /// let lf = b'\n';
    /// let esc = b'\x1b';
    ///
    /// assert!(uppercase_a.is_ascii_hexdigit());
    /// assert!(!uppercase_g.is_ascii_hexdigit());
    /// assert!(a.is_ascii_hexdigit());
    /// assert!(!g.is_ascii_hexdigit());
    /// assert!(zero.is_ascii_hexdigit());
    /// assert!(!percent.is_ascii_hexdigit());
    /// assert!(!space.is_ascii_hexdigit());
    /// assert!(!lf.is_ascii_hexdigit());
    /// assert!(!esc.is_ascii_hexdigit());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_hexdigit(&self) -> bool {
        matches!(*self, b'0'..=b'9') | matches!(*self, b'A'..=b'F') | matches!(*self, b'a'..=b'f')
    }

    /// 检查该值是否为 ASCII 标点字符：
    ///
    /// - U+0021 ..= U+002F `! " # $ % & ' ( ) * + , - . /`，或
    /// - U+003A ..= U+0040 `: ; < = > ? @`，或
    /// - U+005B ..= U+0060 `` [ \ ] ^ _ ` ``，或
    /// - U+007B ..= U+007E `{ | } ~`
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = b'A';
    /// let uppercase_g = b'G';
    /// let a = b'a';
    /// let g = b'g';
    /// let zero = b'0';
    /// let percent = b'%';
    /// let space = b' ';
    /// let lf = b'\n';
    /// let esc = b'\x1b';
    ///
    /// assert!(!uppercase_a.is_ascii_punctuation());
    /// assert!(!uppercase_g.is_ascii_punctuation());
    /// assert!(!a.is_ascii_punctuation());
    /// assert!(!g.is_ascii_punctuation());
    /// assert!(!zero.is_ascii_punctuation());
    /// assert!(percent.is_ascii_punctuation());
    /// assert!(!space.is_ascii_punctuation());
    /// assert!(!lf.is_ascii_punctuation());
    /// assert!(!esc.is_ascii_punctuation());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_punctuation(&self) -> bool {
        matches!(*self, b'!'..=b'/')
            | matches!(*self, b':'..=b'@')
            | matches!(*self, b'['..=b'`')
            | matches!(*self, b'{'..=b'~')
    }

    /// 检查该值是否为 ASCII 图形字符：
    /// U+0021 '!' ..= U+007E '~'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = b'A';
    /// let uppercase_g = b'G';
    /// let a = b'a';
    /// let g = b'g';
    /// let zero = b'0';
    /// let percent = b'%';
    /// let space = b' ';
    /// let lf = b'\n';
    /// let esc = b'\x1b';
    ///
    /// assert!(uppercase_a.is_ascii_graphic());
    /// assert!(uppercase_g.is_ascii_graphic());
    /// assert!(a.is_ascii_graphic());
    /// assert!(g.is_ascii_graphic());
    /// assert!(zero.is_ascii_graphic());
    /// assert!(percent.is_ascii_graphic());
    /// assert!(!space.is_ascii_graphic());
    /// assert!(!lf.is_ascii_graphic());
    /// assert!(!esc.is_ascii_graphic());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_graphic(&self) -> bool {
        matches!(*self, b'!'..=b'~')
    }

    /// 检查该值是否为 ASCII 空白字符：
    /// U+0020 SPACE, U+0009 HORIZONTAL TAB, U+000A LINE FEED,
    /// U+000C FORM FEED，或 U+000D CARRIAGE RETURN。
    ///
    /// Rust 使用 WhatWG Infra Standard 的 [ASCII 空白定义][infra-aw]。
    /// 还有若干其他定义也被广泛使用。例如，[POSIX locale][pct] 除了上述所有字符外
    /// 还包含 U+000B VERTICAL TAB；但同一规范中的
    /// [Bourne shell 的“字段拆分”默认规则][bfs] 只把 SPACE、HORIZONTAL TAB 和
    /// LINE FEED 视为空白。
    ///
    /// 如果你编写的程序要处理既有文件格式，请在使用此函数前确认该格式采用的空白定义。
    ///
    /// [infra-aw]: https://infra.spec.whatwg.org/#ascii-whitespace
    /// [pct]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap07.html#tag_07_03_01
    /// [bfs]: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html#tag_18_06_05
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = b'A';
    /// let uppercase_g = b'G';
    /// let a = b'a';
    /// let g = b'g';
    /// let zero = b'0';
    /// let percent = b'%';
    /// let space = b' ';
    /// let lf = b'\n';
    /// let esc = b'\x1b';
    ///
    /// assert!(!uppercase_a.is_ascii_whitespace());
    /// assert!(!uppercase_g.is_ascii_whitespace());
    /// assert!(!a.is_ascii_whitespace());
    /// assert!(!g.is_ascii_whitespace());
    /// assert!(!zero.is_ascii_whitespace());
    /// assert!(!percent.is_ascii_whitespace());
    /// assert!(space.is_ascii_whitespace());
    /// assert!(lf.is_ascii_whitespace());
    /// assert!(!esc.is_ascii_whitespace());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_whitespace(&self) -> bool {
        matches!(*self, b'\t' | b'\n' | b'\x0C' | b'\r' | b' ')
    }

    /// 检查该值是否为 ASCII 控制字符：
    /// U+0000 NUL ..= U+001F UNIT SEPARATOR，或 U+007F DELETE。
    /// 注意，大多数 ASCII 空白字符都是控制字符，但 SPACE 不是。
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = b'A';
    /// let uppercase_g = b'G';
    /// let a = b'a';
    /// let g = b'g';
    /// let zero = b'0';
    /// let percent = b'%';
    /// let space = b' ';
    /// let lf = b'\n';
    /// let esc = b'\x1b';
    ///
    /// assert!(!uppercase_a.is_ascii_control());
    /// assert!(!uppercase_g.is_ascii_control());
    /// assert!(!a.is_ascii_control());
    /// assert!(!g.is_ascii_control());
    /// assert!(!zero.is_ascii_control());
    /// assert!(!percent.is_ascii_control());
    /// assert!(!space.is_ascii_control());
    /// assert!(lf.is_ascii_control());
    /// assert!(esc.is_ascii_control());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_control(&self) -> bool {
        matches!(*self, b'\0'..=b'\x1F' | b'\x7F')
    }

    /// 返回一个迭代器，产出把该 `u8` 当作 ASCII 字符处理后的转义形式。
    ///
    /// 行为与 [`ascii::escape_default`] 相同。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!("0", b'0'.escape_ascii().to_string());
    /// assert_eq!("\\t", b'\t'.escape_ascii().to_string());
    /// assert_eq!("\\r", b'\r'.escape_ascii().to_string());
    /// assert_eq!("\\n", b'\n'.escape_ascii().to_string());
    /// assert_eq!("\\'", b'\''.escape_ascii().to_string());
    /// assert_eq!("\\\"", b'"'.escape_ascii().to_string());
    /// assert_eq!("\\\\", b'\\'.escape_ascii().to_string());
    /// assert_eq!("\\x9d", b'\x9d'.escape_ascii().to_string());
    /// ```
    #[must_use = "this returns the escaped byte as an iterator, \
                  without modifying the original"]
    #[stable(feature = "inherent_ascii_escape", since = "1.60.0")]
    #[inline]
    pub fn escape_ascii(self) -> ascii::EscapeDefault {
        ascii::escape_default(self)
    }

    #[inline]
    pub(crate) const fn is_utf8_char_boundary(self) -> bool {
        // 这段位运算等价于：b < 128 || b >= 192
        (self as i8) >= -0x40
    }
}

impl u16 {
    uint_impl! {
        Self = u16,
        ActualT = u16,
        SignedT = i16,
        BITS = 16,
        BITS_MINUS_ONE = 15,
        MAX = 65535,
        rot = 4,
        rot_op = "0xa003",
        rot_result = "0x3a",
        fsh_op = "0x2de",
        fshl_result = "0x30",
        fshr_result = "0x302d",
        swap_op = "0x1234",
        swapped = "0x3412",
        reversed = "0x2c48",
        le_bytes = "[0x34, 0x12]",
        be_bytes = "[0x12, 0x34]",
        to_xe_bytes_doc = "",
        from_xe_bytes_doc = "",
        bound_condition = "",
    }
    midpoint_impl! { u16, u32, unsigned }

    /// 检查该值是否为 Unicode 代理码点；这些值不允许作为 [`char`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(utf16_extra)]
    ///
    /// let low_non_surrogate = 0xA000u16;
    /// let low_surrogate = 0xD800u16;
    /// let high_surrogate = 0xDC00u16;
    /// let high_non_surrogate = 0xE000u16;
    ///
    /// assert!(!low_non_surrogate.is_utf16_surrogate());
    /// assert!(low_surrogate.is_utf16_surrogate());
    /// assert!(high_surrogate.is_utf16_surrogate());
    /// assert!(!high_non_surrogate.is_utf16_surrogate());
    /// ```
    #[must_use]
    #[unstable(feature = "utf16_extra", issue = "94919")]
    #[inline]
    pub const fn is_utf16_surrogate(self) -> bool {
        matches!(self, 0xD800..=0xDFFF)
    }
}

impl u32 {
    uint_impl! {
        Self = u32,
        ActualT = u32,
        SignedT = i32,
        BITS = 32,
        BITS_MINUS_ONE = 31,
        MAX = 4294967295,
        rot = 8,
        rot_op = "0x10000b3",
        rot_result = "0xb301",
        fsh_op = "0x2fe78e45",
        fshl_result = "0xb32f",
        fshr_result = "0xb32fe78e",
        swap_op = "0x12345678",
        swapped = "0x78563412",
        reversed = "0x1e6a2c48",
        le_bytes = "[0x78, 0x56, 0x34, 0x12]",
        be_bytes = "[0x12, 0x34, 0x56, 0x78]",
        to_xe_bytes_doc = "",
        from_xe_bytes_doc = "",
        bound_condition = "",
    }
    midpoint_impl! { u32, u64, unsigned }
}

impl u64 {
    uint_impl! {
        Self = u64,
        ActualT = u64,
        SignedT = i64,
        BITS = 64,
        BITS_MINUS_ONE = 63,
        MAX = 18446744073709551615,
        rot = 12,
        rot_op = "0xaa00000000006e1",
        rot_result = "0x6e10aa",
        fsh_op = "0x2fe78e45983acd98",
        fshl_result = "0x6e12fe",
        fshr_result = "0x6e12fe78e45983ac",
        swap_op = "0x1234567890123456",
        swapped = "0x5634129078563412",
        reversed = "0x6a2c48091e6a2c48",
        le_bytes = "[0x56, 0x34, 0x12, 0x90, 0x78, 0x56, 0x34, 0x12]",
        be_bytes = "[0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56]",
        to_xe_bytes_doc = "",
        from_xe_bytes_doc = "",
        bound_condition = "",
    }
    midpoint_impl! { u64, u128, unsigned }
}

impl u128 {
    uint_impl! {
        Self = u128,
        ActualT = u128,
        SignedT = i128,
        BITS = 128,
        BITS_MINUS_ONE = 127,
        MAX = 340282366920938463463374607431768211455,
        rot = 16,
        rot_op = "0x13f40000000000000000000000004f76",
        rot_result = "0x4f7613f4",
        fsh_op = "0x2fe78e45983acd98039000008736273",
        fshl_result = "0x4f7602fe",
        fshr_result = "0x4f7602fe78e45983acd9803900000873",
        swap_op = "0x12345678901234567890123456789012",
        swapped = "0x12907856341290785634129078563412",
        reversed = "0x48091e6a2c48091e6a2c48091e6a2c48",
        le_bytes = "[0x12, 0x90, 0x78, 0x56, 0x34, 0x12, 0x90, 0x78, \
            0x56, 0x34, 0x12, 0x90, 0x78, 0x56, 0x34, 0x12]",
        be_bytes = "[0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56, \
            0x78, 0x90, 0x12, 0x34, 0x56, 0x78, 0x90, 0x12]",
        to_xe_bytes_doc = "",
        from_xe_bytes_doc = "",
        bound_condition = "",
    }
    midpoint_impl! { u128, unsigned }
}

#[cfg(target_pointer_width = "16")]
impl usize {
    uint_impl! {
        Self = usize,
        ActualT = u16,
        SignedT = isize,
        BITS = 16,
        BITS_MINUS_ONE = 15,
        MAX = 65535,
        rot = 4,
        rot_op = "0xa003",
        rot_result = "0x3a",
        fsh_op = "0x2fe78e45983acd98039000008736273",
        fshl_result = "0x4f7602fe",
        fshr_result = "0x4f7602fe78e45983acd9803900000873",
        swap_op = "0x1234",
        swapped = "0x3412",
        reversed = "0x2c48",
        le_bytes = "[0x34, 0x12]",
        be_bytes = "[0x12, 0x34]",
        to_xe_bytes_doc = usize_isize_to_xe_bytes_doc!(),
        from_xe_bytes_doc = usize_isize_from_xe_bytes_doc!(),
        bound_condition = " on 16-bit targets",
    }
    midpoint_impl! { usize, u32, unsigned }
}

#[cfg(target_pointer_width = "32")]
impl usize {
    uint_impl! {
        Self = usize,
        ActualT = u32,
        SignedT = isize,
        BITS = 32,
        BITS_MINUS_ONE = 31,
        MAX = 4294967295,
        rot = 8,
        rot_op = "0x10000b3",
        rot_result = "0xb301",
        fsh_op = "0x2fe78e45",
        fshl_result = "0xb32f",
        fshr_result = "0xb32fe78e",
        swap_op = "0x12345678",
        swapped = "0x78563412",
        reversed = "0x1e6a2c48",
        le_bytes = "[0x78, 0x56, 0x34, 0x12]",
        be_bytes = "[0x12, 0x34, 0x56, 0x78]",
        to_xe_bytes_doc = usize_isize_to_xe_bytes_doc!(),
        from_xe_bytes_doc = usize_isize_from_xe_bytes_doc!(),
        bound_condition = " on 32-bit targets",
    }
    midpoint_impl! { usize, u64, unsigned }
}

#[cfg(target_pointer_width = "64")]
impl usize {
    uint_impl! {
        Self = usize,
        ActualT = u64,
        SignedT = isize,
        BITS = 64,
        BITS_MINUS_ONE = 63,
        MAX = 18446744073709551615,
        rot = 12,
        rot_op = "0xaa00000000006e1",
        rot_result = "0x6e10aa",
        fsh_op = "0x2fe78e45983acd98",
        fshl_result = "0x6e12fe",
        fshr_result = "0x6e12fe78e45983ac",
        swap_op = "0x1234567890123456",
        swapped = "0x5634129078563412",
        reversed = "0x6a2c48091e6a2c48",
        le_bytes = "[0x56, 0x34, 0x12, 0x90, 0x78, 0x56, 0x34, 0x12]",
        be_bytes = "[0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56]",
        to_xe_bytes_doc = usize_isize_to_xe_bytes_doc!(),
        from_xe_bytes_doc = usize_isize_from_xe_bytes_doc!(),
        bound_condition = " on 64-bit targets",
    }
    midpoint_impl! { usize, u128, unsigned }
}

impl usize {
    /// 返回一个每个字节都等于 `x` 的 `usize`。
    #[inline]
    pub(crate) const fn repeat_u8(x: u8) -> usize {
        usize::from_ne_bytes([x; size_of::<usize>()])
    }

    /// 返回一个每个双字节单元都等于 `x` 的 `usize`。
    #[inline]
    pub(crate) const fn repeat_u16(x: u16) -> usize {
        let mut r = 0usize;
        let mut i = 0;
        while i < size_of::<usize>() {
            // 使用 `wrapping_shl`，使其能在 `usize` 为 16 位的目标上工作。
            r = r.wrapping_shl(16) | (x as usize);
            i += 2;
        }
        r
    }
}

/// 浮点数的分类。
///
/// 此 `enum` 用作 [`f32::classify`] 和 [`f64::classify`] 的返回类型。
/// 更多信息请参见它们的文档。
///
/// # 示例
///
/// ```
/// use std::num::FpCategory;
///
/// let num = 12.4_f32;
/// let inf = f32::INFINITY;
/// let zero = 0f32;
/// let sub: f32 = 1.1754942e-38;
/// let nan = f32::NAN;
///
/// assert_eq!(num.classify(), FpCategory::Normal);
/// assert_eq!(inf.classify(), FpCategory::Infinite);
/// assert_eq!(zero.classify(), FpCategory::Zero);
/// assert_eq!(sub.classify(), FpCategory::Subnormal);
/// assert_eq!(nan.classify(), FpCategory::Nan);
/// ```
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub enum FpCategory {
    /// NaN（非数）：这个值来自 `(-1.0).sqrt()` 这类计算。
    ///
    /// 更多关于 NaN 异常性质的信息，请参见 [`f32` 的文档](f32)。
    #[stable(feature = "rust1", since = "1.0.0")]
    Nan,

    /// 正无穷或负无穷，常由非零数除以零产生。
    #[stable(feature = "rust1", since = "1.0.0")]
    Infinite,

    /// 正零或负零。
    ///
    /// 更多关于零的符号性的信息，请参见 [`f32` 的文档](f32)。
    #[stable(feature = "rust1", since = "1.0.0")]
    Zero,

    /// “次正规”或“非正规”浮点表示；相对于其量级，它比 [`Normal`] 精度更低。
    ///
    /// 次正规数的量级大于 [`Zero`]，但小于所有 [`Normal`] 数的量级。
    ///
    /// [`Normal`]: Self::Normal
    /// [`Zero`]: Self::Zero
    #[stable(feature = "rust1", since = "1.0.0")]
    Subnormal,

    /// 常规浮点数，不属于任何异常分类。
    ///
    /// 最小的正规正数是 [`f32::MIN_POSITIVE`] 和 [`f64::MIN_POSITIVE`]，
    /// 最大的正规正数是 [`f32::MAX`] 和 [`f64::MAX`]。（与有符号整数不同，
    /// 浮点数的取值范围是对称的，因此对其中任一常量取负会得到对应的负数。）
    #[stable(feature = "rust1", since = "1.0.0")]
    Normal,
}

/// 判断给定进制和长度的文本串是否可以保证存入给定类型 `T`。
/// 注意：如果 `radix` 对编译器已知，运行时只需要检查 `digits.len`。
#[doc(hidden)]
#[inline(always)]
#[unstable(issue = "none", feature = "std_internals")]
pub const fn can_not_overflow<T>(radix: u32, is_signed_ty: bool, digits: &[u8]) -> bool {
    radix <= 16 && digits.len() <= size_of::<T>() * 2 - is_signed_ty as usize
}

#[cfg_attr(not(panic = "immediate-abort"), inline(never))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[cold]
#[track_caller]
const fn from_ascii_radix_panic(radix: u32) -> ! {
    const_panic!(
        "from_ascii_radix: radix must lie in the range `[2, 36]`",
        "from_ascii_radix: radix must lie in the range `[2, 36]` - found {radix}",
        radix: u32 = radix,
    )
}

macro_rules! from_str_int_impl {
    ($signedness:ident $($int_ty:ty)+) => {$(
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
        impl const FromStr for $int_ty {
            type Err = ParseIntError;

            /// 从包含十进制数字的字符串切片解析整数。
            ///
            /// 字符预期为一个可选的
            #[doc = sign_dependent_expr!{
                $signedness ?
                if signed {
                    " `+` or `-` "
                }
                if unsigned {
                    " `+` "
                }
            }]
            /// 符号，后面只跟数字。前后出现非数字字符（包括空白）表示错误。
            /// 下划线（Rust 字面量中允许）同样表示错误。
            ///
            /// # 另请参阅
            /// 若要解析其他进制（如二进制或十六进制）的数字，
            /// 请参见 [`from_str_radix`][Self::from_str_radix]。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::str::FromStr;
            ///
            #[doc = concat!("assert_eq!(", stringify!($int_ty), "::from_str(\"+10\"), Ok(10));")]
            /// ```
            /// 尾随空格会返回错误：
            /// ```
            /// # use std::str::FromStr;
            /// #
            #[doc = concat!("assert!(", stringify!($int_ty), "::from_str(\"1 \").is_err());")]
            /// ```
            #[inline]
            fn from_str(src: &str) -> Result<$int_ty, ParseIntError> {
                <$int_ty>::from_str_radix(src, 10)
            }
        }

        impl $int_ty {
            /// 从字符串切片中按给定进制解析整数。
            ///
            /// 字符串预期为一个可选的
            #[doc = sign_dependent_expr!{
                $signedness ?
                if signed {
                    " `+` or `-` "
                }
                if unsigned {
                    " `+` "
                }
            }]
            /// 符号，后面只跟数字。前后出现非数字字符（包括空白）表示错误。
            /// 下划线（Rust 字面量中允许）同样表示错误。
            ///
            /// 数字字符的集合取决于 `radix`，是下列字符的子集：
            /// * `0-9`
            /// * `a-z`
            /// * `A-Z`
            ///
            /// # Panics
            ///
            /// 如果 `radix` 不在 2 到 36 的范围内，此函数会 panic。
            ///
            /// # 另请参阅
            /// 如果要解析的字符串是十进制，也可以使用
            /// [`from_str`] 或 [`str::parse`]。
            ///
            // FIXME(#122566): 这些 HTML 链接用于绕开一个 rustdoc-json 测试失败。
            /// [`from_str`]: #method.from_str
            /// [`str::parse`]: primitive.str.html#method.parse
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!("assert_eq!(", stringify!($int_ty), "::from_str_radix(\"A\", 16), Ok(10));")]
            /// ```
            /// 尾随空格会返回错误：
            /// ```
            #[doc = concat!("assert!(", stringify!($int_ty), "::from_str_radix(\"1 \", 10).is_err());")]
            /// ```
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_stable(feature = "const_int_from_str", since = "1.82.0")]
            #[inline]
            pub const fn from_str_radix(src: &str, radix: u32) -> Result<$int_ty, ParseIntError> {
                <$int_ty>::from_ascii_radix(src.as_bytes(), radix)
            }

            /// 从包含十进制数字的 ASCII 字节切片解析整数。
            ///
            /// 这些字节预期表示一个可选的
            #[doc = sign_dependent_expr!{
                $signedness ?
                if signed {
                    " `+` or `-` "
                }
                if unsigned {
                    " `+` "
                }
            }]
            /// 符号，后面只跟数字。前后出现非数字字符（包括空白）表示错误。
            /// 下划线（Rust 字面量中允许）同样表示错误。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(int_from_ascii)]
            ///
            #[doc = concat!("assert_eq!(", stringify!($int_ty), "::from_ascii(b\"+10\"), Ok(10));")]
            /// ```
            /// 尾随空格会返回错误：
            /// ```
            /// # #![feature(int_from_ascii)]
            /// #
            #[doc = concat!("assert!(", stringify!($int_ty), "::from_ascii(b\"1 \").is_err());")]
            /// ```
            #[unstable(feature = "int_from_ascii", issue = "134821")]
            #[inline]
            pub const fn from_ascii(src: &[u8]) -> Result<$int_ty, ParseIntError> {
                <$int_ty>::from_ascii_radix(src, 10)
            }

            /// 从 ASCII 字节切片中按给定进制解析整数。
            ///
            /// 这些字节预期表示一个可选的
            #[doc = sign_dependent_expr!{
                $signedness ?
                if signed {
                    " `+` or `-` "
                }
                if unsigned {
                    " `+` "
                }
            }]
            /// 符号，后面只跟数字。前后出现非数字字符（包括空白）表示错误。
            /// 下划线（Rust 字面量中允许）同样表示错误。
            ///
            /// 数字字符的集合取决于 `radix`，是下列字符的子集：
            /// * `0-9`
            /// * `a-z`
            /// * `A-Z`
            ///
            /// # Panics
            ///
            /// 如果 `radix` 不在 2 到 36 的范围内，此函数会 panic。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(int_from_ascii)]
            ///
            #[doc = concat!("assert_eq!(", stringify!($int_ty), "::from_ascii_radix(b\"A\", 16), Ok(10));")]
            /// ```
            /// 尾随空格会返回错误：
            /// ```
            /// # #![feature(int_from_ascii)]
            /// #
            #[doc = concat!("assert!(", stringify!($int_ty), "::from_ascii_radix(b\"1 \", 10).is_err());")]
            /// ```
            #[unstable(feature = "int_from_ascii", issue = "134821")]
            #[inline]
            pub const fn from_ascii_radix(src: &[u8], radix: u32) -> Result<$int_ty, ParseIntError> {
                use self::IntErrorKind::*;
                use self::ParseIntError as PIE;

                if 2 > radix || radix > 36 {
                    from_ascii_radix_panic(radix);
                }

                if src.is_empty() {
                    return Err(PIE { kind: Empty });
                }

                #[allow(unused_comparisons)]
                let is_signed_ty = 0 > <$int_ty>::MIN;

                let (is_positive, mut digits) = match src {
                    [b'+' | b'-'] => {
                        return Err(PIE { kind: InvalidDigit });
                    }
                    [b'+', rest @ ..] => (true, rest),
                    [b'-', rest @ ..] if is_signed_ty => (false, rest),
                    _ => (true, src),
                };

                let mut result = 0;

                macro_rules! unwrap_or_PIE {
                    ($option:expr, $kind:ident) => {
                        match $option {
                            Some(value) => value,
                            None => return Err(PIE { kind: $kind }),
                        }
                    };
                }

                if can_not_overflow::<$int_ty>(radix, is_signed_ty, digits) {
                    // 如果字符串长度相对于要解析到的类型范围足够短，
                    // 就可以确定不会发生溢出。
                    // 精确边界是 `radix.pow(digits.len()) - 1 <= T::MAX`，
                    // 但上面的条件是更快的保守近似。
                    //
                    // 以 16 进制为例：它每个数字的信息密度最高，因此会最早溢出：
                    // `u8::MAX` 是 `ff`，任何长度为 2 的字符串都保证不会溢出。
                    // `i8::MAX` 是 `7f`，只有长度为 1 的字符串保证不会溢出。
                    macro_rules! run_unchecked_loop {
                        ($unchecked_additive_op:tt) => {{
                            while let [c, rest @ ..] = digits {
                                result = result * (radix as $int_ty);
                                let x = unwrap_or_PIE!((*c as char).to_digit(radix), InvalidDigit);
                                result = result $unchecked_additive_op (x as $int_ty);
                                digits = rest;
                            }
                        }};
                    }
                    if is_positive {
                        run_unchecked_loop!(+)
                    } else {
                        run_unchecked_loop!(-)
                    };
                } else {
                    macro_rules! run_checked_loop {
                        ($checked_additive_op:ident, $overflow_err:ident) => {{
                            while let [c, rest @ ..] = digits {
                                // 当 `radix` 以字面量传入时，如果 `radix` 可以表示为
                                // 若干 2 的幂之和，编译器就能使用移位而不是较慢的 `imul`
                                //（x*10 可以写成 x*8 + x*2）。
                                // 当编译器无法使用这些优化时，可以在需要结果之前先发出乘法，
                                // 隐藏乘法延迟，从而提升现代乱序执行 CPU 上的性能。
                                // 这里乘法比其他指令更慢；先做乘法，让 CPU 用其他周期执行
                                // 其他计算，稍后再取得乘法结果，可以更快得到最终结果。
                                let mul = result.checked_mul(radix as $int_ty);
                                let x = unwrap_or_PIE!((*c as char).to_digit(radix), InvalidDigit) as $int_ty;
                                result = unwrap_or_PIE!(mul, $overflow_err);
                                result = unwrap_or_PIE!(<$int_ty>::$checked_additive_op(result, x), $overflow_err);
                                digits = rest;
                            }
                        }};
                    }
                    if is_positive {
                        run_checked_loop!(checked_add, PosOverflow)
                    } else {
                        run_checked_loop!(checked_sub, NegOverflow)
                    };
                }
                Ok(result)
            }
        }
    )*}
}

from_str_int_impl! { signed isize i8 i16 i32 i64 i128 }
from_str_int_impl! { unsigned usize u8 u16 u32 u64 u128 }
