macro_rules! int_impl {
    (
        Self = $SelfT:ty,
        ActualT = $ActualT:ident,
        UnsignedT = $UnsignedT:ty,

        // 这些只用于文档注释。
        // 因此它们都会以字面量传入；如果需要表示多个代码 token，
        // 传入字符串字面量也是可以的。
        // 在非注释中，请使用关联常量而不是这些值。
        BITS = $BITS:literal,
        BITS_MINUS_ONE = $BITS_MINUS_ONE:literal,
        Min = $Min:literal,
        Max = $Max:literal,
        rot = $rot:literal,
        rot_op = $rot_op:literal,
        rot_result = $rot_result:literal,
        swap_op = $swap_op:literal,
        swapped = $swapped:literal,
        reversed = $reversed:literal,
        le_bytes = $le_bytes:literal,
        be_bytes = $be_bytes:literal,
        to_xe_bytes_doc = $to_xe_bytes_doc:expr,
        from_xe_bytes_doc = $from_xe_bytes_doc:expr,
        bound_condition = $bound_condition:literal,
    ) => {
        /// 此整数类型能表示的最小值
        #[doc = concat!("(&minus;2<sup>", $BITS_MINUS_ONE, "</sup>", $bound_condition, ").")]
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN, ", stringify!($Min), ");")]
        /// ```
        #[stable(feature = "assoc_int_consts", since = "1.43.0")]
        pub const MIN: Self = !Self::MAX;

        /// 此整数类型能表示的最大值
        #[doc = concat!("(2<sup>", $BITS_MINUS_ONE, "</sup> &minus; 1", $bound_condition, ").")]
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX, ", stringify!($Max), ");")]
        /// ```
        #[stable(feature = "assoc_int_consts", since = "1.43.0")]
        pub const MAX: Self = (<$UnsignedT>::MAX >> 1) as Self;

        /// 此整数类型的大小，以位为单位。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::BITS, ", stringify!($BITS), ");")]
        /// ```
        #[stable(feature = "int_bits_const", since = "1.53.0")]
        pub const BITS: u32 = <$UnsignedT>::BITS;

        /// 返回 `self` 的二进制表示中 1 的个数。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = 0b100_0000", stringify!($SelfT), ";")]
        ///
        /// assert_eq!(n.count_ones(), 1);
        /// ```
        ///
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[doc(alias = "popcount")]
        #[doc(alias = "popcnt")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn count_ones(self) -> u32 { (self as $UnsignedT).count_ones() }

        /// 返回 `self` 的二进制表示中 0 的个数。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.count_zeros(), 1);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn count_zeros(self) -> u32 {
            (!self).count_ones()
        }

        /// 返回 `self` 的二进制表示中前导 0 的个数。
        ///
        /// 根据你对该值的用途，你可能也会需要 [`ilog2`] 函数；
        /// 即使类型变宽，它也会返回一致的数值。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = -1", stringify!($SelfT), ";")]
        ///
        /// assert_eq!(n.leading_zeros(), 0);
        /// ```
        #[doc = concat!("[`ilog2`]: ", stringify!($SelfT), "::ilog2")]
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn leading_zeros(self) -> u32 {
            (self as $UnsignedT).leading_zeros()
        }

        /// 返回 `self` 的二进制表示中尾随 0 的个数。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = -4", stringify!($SelfT), ";")]
        ///
        /// assert_eq!(n.trailing_zeros(), 2);
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn trailing_zeros(self) -> u32 {
            (self as $UnsignedT).trailing_zeros()
        }

        /// 返回 `self` 的二进制表示中前导 1 的个数。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = -1", stringify!($SelfT), ";")]
        ///
        #[doc = concat!("assert_eq!(n.leading_ones(), ", stringify!($BITS), ");")]
        /// ```
        #[stable(feature = "leading_trailing_ones", since = "1.46.0")]
        #[rustc_const_stable(feature = "leading_trailing_ones", since = "1.46.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn leading_ones(self) -> u32 {
            (self as $UnsignedT).leading_ones()
        }

        /// 返回 `self` 的二进制表示中尾随 1 的个数。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = 3", stringify!($SelfT), ";")]
        ///
        /// assert_eq!(n.trailing_ones(), 2);
        /// ```
        #[stable(feature = "leading_trailing_ones", since = "1.46.0")]
        #[rustc_const_stable(feature = "leading_trailing_ones", since = "1.46.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn trailing_ones(self) -> u32 {
            (self as $UnsignedT).trailing_ones()
        }

        /// 返回只保留最高有效位为 1 的 `self`；如果输入为 `0`，
        /// 则返回 `0`。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(isolate_most_least_significant_one)]
        ///
        #[doc = concat!("let n: ", stringify!($SelfT), " = 0b_01100100;")]
        ///
        /// assert_eq!(n.isolate_highest_one(), 0b_01000000);
        #[doc = concat!("assert_eq!(0_", stringify!($SelfT), ".isolate_highest_one(), 0);")]
        /// ```
        #[unstable(feature = "isolate_most_least_significant_one", issue = "136909")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn isolate_highest_one(self) -> Self {
            self & (((1 as $SelfT) << (<$SelfT>::BITS - 1)).wrapping_shr(self.leading_zeros()))
        }

        /// 返回只保留最低有效位为 1 的 `self`；如果输入为 `0`，
        /// 则返回 `0`。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(isolate_most_least_significant_one)]
        ///
        #[doc = concat!("let n: ", stringify!($SelfT), " = 0b_01100100;")]
        ///
        /// assert_eq!(n.isolate_lowest_one(), 0b_00000100);
        #[doc = concat!("assert_eq!(0_", stringify!($SelfT), ".isolate_lowest_one(), 0);")]
        /// ```
        #[unstable(feature = "isolate_most_least_significant_one", issue = "136909")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn isolate_lowest_one(self) -> Self {
            self & self.wrapping_neg()
        }

        /// 返回 `self` 中值为 1 的最高位索引；如果 `self` 为 `0`，
        /// 则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(int_lowest_highest_one)]
        ///
        #[doc = concat!("assert_eq!(0b0_", stringify!($SelfT), ".highest_one(), None);")]
        #[doc = concat!("assert_eq!(0b1_", stringify!($SelfT), ".highest_one(), Some(0));")]
        #[doc = concat!("assert_eq!(0b1_0000_", stringify!($SelfT), ".highest_one(), Some(4));")]
        #[doc = concat!("assert_eq!(0b1_1111_", stringify!($SelfT), ".highest_one(), Some(4));")]
        /// ```
        #[unstable(feature = "int_lowest_highest_one", issue = "145203")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn highest_one(self) -> Option<u32> {
            (self as $UnsignedT).highest_one()
        }

        /// 返回 `self` 中值为 1 的最低位索引；如果 `self` 为 `0`，
        /// 则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(int_lowest_highest_one)]
        ///
        #[doc = concat!("assert_eq!(0b0_", stringify!($SelfT), ".lowest_one(), None);")]
        #[doc = concat!("assert_eq!(0b1_", stringify!($SelfT), ".lowest_one(), Some(0));")]
        #[doc = concat!("assert_eq!(0b1_0000_", stringify!($SelfT), ".lowest_one(), Some(4));")]
        #[doc = concat!("assert_eq!(0b1_1111_", stringify!($SelfT), ".lowest_one(), Some(0));")]
        /// ```
        #[unstable(feature = "int_lowest_highest_one", issue = "145203")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn lowest_one(self) -> Option<u32> {
            (self as $UnsignedT).lowest_one()
        }

        /// 返回将 `self` 的位模式重新解释为同大小无符号整数后的值。
        ///
        /// 这会产生与 `as` 转换相同的结果，但会确保位宽保持不变。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = -1", stringify!($SelfT), ";")]
        ///
        #[doc = concat!("assert_eq!(n.cast_unsigned(), ", stringify!($UnsignedT), "::MAX);")]
        /// ```
        #[stable(feature = "integer_sign_cast", since = "1.87.0")]
        #[rustc_const_stable(feature = "integer_sign_cast", since = "1.87.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn cast_unsigned(self) -> $UnsignedT {
            self as $UnsignedT
        }

        /// 将位向左旋转指定数量 `n`，被截断的位会回绕到结果整数的末端。
        ///
        /// `rotate_left(n)` 等价于总共应用 `n` 次 `rotate_left(1)`。
        /// 特别是，当旋转量等于 `self` 的位数时，会原样返回输入值。
        ///
        /// 请注意，这与 `<<` 移位运算符不是同一个操作！
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = ", $rot_op, stringify!($SelfT), ";")]
        #[doc = concat!("let m = ", $rot_result, ";")]
        ///
        #[doc = concat!("assert_eq!(n.rotate_left(", $rot, "), m);")]
        #[doc = concat!("assert_eq!(n.rotate_left(1024), n);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn rotate_left(self, n: u32) -> Self {
            (self as $UnsignedT).rotate_left(n) as Self
        }

        /// 将位向右旋转指定数量 `n`，被截断的位会回绕到结果整数的开头。
        ///
        /// `rotate_right(n)` 等价于总共应用 `n` 次 `rotate_right(1)`。
        /// 特别是，当旋转量等于 `self` 的位数时，会原样返回输入值。
        ///
        /// 请注意，这与 `>>` 移位运算符不是同一个操作！
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = ", $rot_result, stringify!($SelfT), ";")]
        #[doc = concat!("let m = ", $rot_op, ";")]
        ///
        #[doc = concat!("assert_eq!(n.rotate_right(", $rot, "), m);")]
        #[doc = concat!("assert_eq!(n.rotate_right(1024), n);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn rotate_right(self, n: u32) -> Self {
            (self as $UnsignedT).rotate_right(n) as Self
        }

        /// 反转该整数的字节顺序。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = ", $swap_op, stringify!($SelfT), ";")]
        ///
        /// let m = n.swap_bytes();
        ///
        #[doc = concat!("assert_eq!(m, ", $swapped, ");")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn swap_bytes(self) -> Self {
            (self as $UnsignedT).swap_bytes() as Self
        }

        /// 反转该整数中的位顺序。最低有效位会变为最高有效位，
        /// 次低有效位会变为次高有效位，依此类推。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = ", $swap_op, stringify!($SelfT), ";")]
        /// let m = n.reverse_bits();
        ///
        #[doc = concat!("assert_eq!(m, ", $reversed, ");")]
        #[doc = concat!("assert_eq!(0, 0", stringify!($SelfT), ".reverse_bits());")]
        /// ```
        #[stable(feature = "reverse_bits", since = "1.37.0")]
        #[rustc_const_stable(feature = "reverse_bits", since = "1.37.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn reverse_bits(self) -> Self {
            (self as $UnsignedT).reverse_bits() as Self
        }

        /// 将整数从大端字节序转换为目标平台的字节序。
        ///
        /// 在大端平台上这是空操作。在小端平台上会交换字节。
        ///
        /// 另请参见 [from_be_bytes()](Self::from_be_bytes)。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = 0x1A", stringify!($SelfT), ";")]
        ///
        /// if cfg!(target_endian = "big") {
        #[doc = concat!("    assert_eq!(", stringify!($SelfT), "::from_be(n), n)")]
        /// } else {
        #[doc = concat!("    assert_eq!(", stringify!($SelfT), "::from_be(n), n.swap_bytes())")]
        /// }
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_conversions", since = "1.32.0")]
        #[must_use]
        #[inline]
        pub const fn from_be(x: Self) -> Self {
            #[cfg(target_endian = "big")]
            {
                x
            }
            #[cfg(not(target_endian = "big"))]
            {
                x.swap_bytes()
            }
        }

        /// 将整数从小端字节序转换为目标平台的字节序。
        ///
        /// 在小端平台上这是空操作。在大端平台上会交换字节。
        ///
        /// 另请参见 [from_le_bytes()](Self::from_le_bytes)。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = 0x1A", stringify!($SelfT), ";")]
        ///
        /// if cfg!(target_endian = "little") {
        #[doc = concat!("    assert_eq!(", stringify!($SelfT), "::from_le(n), n)")]
        /// } else {
        #[doc = concat!("    assert_eq!(", stringify!($SelfT), "::from_le(n), n.swap_bytes())")]
        /// }
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_conversions", since = "1.32.0")]
        #[must_use]
        #[inline]
        pub const fn from_le(x: Self) -> Self {
            #[cfg(target_endian = "little")]
            {
                x
            }
            #[cfg(not(target_endian = "little"))]
            {
                x.swap_bytes()
            }
        }

        /// 在小端目标平台上交换 `self` 的字节。
        ///
        /// 在大端平台上这是空操作。
        ///
        /// 返回值与 `self` 具有相同类型，并会被解释为一个
        /// 本机字节序的（可能不同的）值：
        #[doc = concat!("`", stringify!($SelfT), "`.")]
        ///
        /// 如需类型安全的替代方案，请参见 [`to_be_bytes()`](Self::to_be_bytes)。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = 0x1A", stringify!($SelfT), ";")]
        ///
        /// if cfg!(target_endian = "big") {
        ///     assert_eq!(n.to_be(), n)
        /// } else {
        ///     assert_eq!(n.to_be(), n.swap_bytes())
        /// }
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_conversions", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn to_be(self) -> Self { // 或者不转成大端？
            #[cfg(target_endian = "big")]
            {
                self
            }
            #[cfg(not(target_endian = "big"))]
            {
                self.swap_bytes()
            }
        }

        /// 在大端目标平台上交换 `self` 的字节。
        ///
        /// 在小端平台上这是空操作。
        ///
        /// 返回值与 `self` 具有相同类型，并会被解释为一个
        /// 本机字节序的（可能不同的）值：
        #[doc = concat!("`", stringify!($SelfT), "`.")]
        ///
        /// 如需类型安全的替代方案，请参见 [`to_le_bytes()`](Self::to_le_bytes)。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = 0x1A", stringify!($SelfT), ";")]
        ///
        /// if cfg!(target_endian = "little") {
        ///     assert_eq!(n.to_le(), n)
        /// } else {
        ///     assert_eq!(n.to_le(), n.swap_bytes())
        /// }
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_conversions", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn to_le(self) -> Self {
            #[cfg(target_endian = "little")]
            {
                self
            }
            #[cfg(not(target_endian = "little"))]
            {
                self.swap_bytes()
            }
        }

        /// 检查型整数加法。计算 `self + rhs`，如果发生溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).checked_add(1), Some(", stringify!($SelfT), "::MAX - 1));")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).checked_add(3), None);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_checked_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_add(self, rhs: Self) -> Option<Self> {
            let (a, b) = self.overflowing_add(rhs);
            if intrinsics::unlikely(b) { None } else { Some(a) }
        }

        /// 严格整数加法。计算 `self + rhs`，如果发生溢出则 panic。
        ///
        /// # Panics
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).strict_add(1), ", stringify!($SelfT), "::MAX - 1);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = (", stringify!($SelfT), "::MAX - 2).strict_add(3);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_add(self, rhs: Self) -> Self {
            let (a, b) = self.overflowing_add(rhs);
            if b { overflow_panic::add() } else { a }
        }

        /// 不检查的整数加法。计算 `self + rhs`，并假定不会发生溢出。
        ///
        /// 调用 `x.unchecked_add(y)` 在语义上等价于调用
        /// `x.`[`checked_add`]`(y).`[`unwrap_unchecked`]`()`.
        ///
        /// 如果你只是想避免调试模式下的 panic，那么**不要**使用此函数。
        /// 你需要的是 [`wrapping_add`]。
        ///
        /// # 安全性(Safety）
        ///
        /// 当出现以下情况时会导致未定义行为：
        #[doc = concat!("`self + rhs > ", stringify!($SelfT), "::MAX` or `self + rhs < ", stringify!($SelfT), "::MIN`,")]
        /// 也就是 [`checked_add`] 会返回 `None` 的情况。
        ///
        /// [`unwrap_unchecked`]: option/enum.Option.html#method.unwrap_unchecked
        #[doc = concat!("[`checked_add`]: ", stringify!($SelfT), "::checked_add")]
        #[doc = concat!("[`wrapping_add`]: ", stringify!($SelfT), "::wrapping_add")]
        #[stable(feature = "unchecked_math", since = "1.79.0")]
        #[rustc_const_stable(feature = "unchecked_math", since = "1.79.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const unsafe fn unchecked_add(self, rhs: Self) -> Self {
            assert_unsafe_precondition!(
                check_language_ub,
                concat!(stringify!($SelfT), "::unchecked_add cannot overflow"),
                (
                    lhs: $SelfT = self,
                    rhs: $SelfT = rhs,
                ) => !lhs.overflowing_add(rhs).1,
            );

            // SAFETY: 调用方必须保证加法不会溢出，这是 `unchecked_add` 的前置条件。
            unsafe {
                intrinsics::unchecked_add(self, rhs)
            }
        }

        /// 与无符号整数相加的检查型加法。计算 `self + rhs`，
        /// 如果发生溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".checked_add_unsigned(2), Some(3));")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).checked_add_unsigned(3), None);")]
        /// ```
        #[stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_add_unsigned(self, rhs: $UnsignedT) -> Option<Self> {
            let (a, b) = self.overflowing_add_unsigned(rhs);
            if intrinsics::unlikely(b) { None } else { Some(a) }
        }

        /// 与无符号整数相加的严格加法。计算 `self + rhs`，
        /// 如果发生溢出则 panic。
        ///
        /// # Panics
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".strict_add_unsigned(2), 3);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = (", stringify!($SelfT), "::MAX - 2).strict_add_unsigned(3);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_add_unsigned(self, rhs: $UnsignedT) -> Self {
            let (a, b) = self.overflowing_add_unsigned(rhs);
            if b { overflow_panic::add() } else { a }
        }

        /// 检查型整数减法。计算 `self - rhs`，如果发生溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN + 2).checked_sub(1), Some(", stringify!($SelfT), "::MIN + 1));")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN + 2).checked_sub(3), None);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_checked_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
            let (a, b) = self.overflowing_sub(rhs);
            if intrinsics::unlikely(b) { None } else { Some(a) }
        }

        /// 严格整数减法。计算 `self - rhs`，如果发生溢出则 panic。
        ///
        /// # Panics
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN + 2).strict_sub(1), ", stringify!($SelfT), "::MIN + 1);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = (", stringify!($SelfT), "::MIN + 2).strict_sub(3);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_sub(self, rhs: Self) -> Self {
            let (a, b) = self.overflowing_sub(rhs);
            if b { overflow_panic::sub() } else { a }
        }

        /// 不检查的整数减法。计算 `self - rhs`，并假定不会发生溢出。
        ///
        /// 调用 `x.unchecked_sub(y)` 在语义上等价于调用
        /// `x.`[`checked_sub`]`(y).`[`unwrap_unchecked`]`()`.
        ///
        /// 如果你只是想避免调试模式下的 panic，那么**不要**使用此函数。
        /// 你需要的是 [`wrapping_sub`]。
        ///
        /// # 安全性(Safety）
        ///
        /// 当出现以下情况时会导致未定义行为：
        #[doc = concat!("`self - rhs > ", stringify!($SelfT), "::MAX` or `self - rhs < ", stringify!($SelfT), "::MIN`,")]
        /// 也就是 [`checked_sub`] 会返回 `None` 的情况。
        ///
        /// [`unwrap_unchecked`]: option/enum.Option.html#method.unwrap_unchecked
        #[doc = concat!("[`checked_sub`]: ", stringify!($SelfT), "::checked_sub")]
        #[doc = concat!("[`wrapping_sub`]: ", stringify!($SelfT), "::wrapping_sub")]
        #[stable(feature = "unchecked_math", since = "1.79.0")]
        #[rustc_const_stable(feature = "unchecked_math", since = "1.79.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const unsafe fn unchecked_sub(self, rhs: Self) -> Self {
            assert_unsafe_precondition!(
                check_language_ub,
                concat!(stringify!($SelfT), "::unchecked_sub cannot overflow"),
                (
                    lhs: $SelfT = self,
                    rhs: $SelfT = rhs,
                ) => !lhs.overflowing_sub(rhs).1,
            );

            // SAFETY: 调用方必须保证减法不会溢出，这是 `unchecked_sub` 的前置条件。
            unsafe {
                intrinsics::unchecked_sub(self, rhs)
            }
        }

        /// 与无符号整数相减的检查型减法。计算 `self - rhs`，
        /// 如果发生溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".checked_sub_unsigned(2), Some(-1));")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN + 2).checked_sub_unsigned(3), None);")]
        /// ```
        #[stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_sub_unsigned(self, rhs: $UnsignedT) -> Option<Self> {
            let (a, b) = self.overflowing_sub_unsigned(rhs);
            if intrinsics::unlikely(b) { None } else { Some(a) }
        }

        /// 与无符号整数相减的严格减法。计算 `self - rhs`，
        /// 如果发生溢出则 panic。
        ///
        /// # Panics
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".strict_sub_unsigned(2), -1);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = (", stringify!($SelfT), "::MIN + 2).strict_sub_unsigned(3);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_sub_unsigned(self, rhs: $UnsignedT) -> Self {
            let (a, b) = self.overflowing_sub_unsigned(rhs);
            if b { overflow_panic::sub() } else { a }
        }

        /// 检查型整数乘法。计算 `self * rhs`，如果发生溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.checked_mul(1), Some(", stringify!($SelfT), "::MAX));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.checked_mul(2), None);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_checked_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_mul(self, rhs: Self) -> Option<Self> {
            let (a, b) = self.overflowing_mul(rhs);
            if intrinsics::unlikely(b) { None } else { Some(a) }
        }

        /// 严格整数乘法。计算 `self * rhs`，如果发生溢出则 panic。
        ///
        /// # Panics
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.strict_mul(1), ", stringify!($SelfT), "::MAX);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ``` should_panic
        #[doc = concat!("let _ = ", stringify!($SelfT), "::MAX.strict_mul(2);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_mul(self, rhs: Self) -> Self {
            let (a, b) = self.overflowing_mul(rhs);
            if b { overflow_panic::mul() } else { a }
        }

        /// 不检查的整数乘法。计算 `self * rhs`，并假定不会发生溢出。
        ///
        /// 调用 `x.unchecked_mul(y)` 在语义上等价于调用
        /// `x.`[`checked_mul`]`(y).`[`unwrap_unchecked`]`()`.
        ///
        /// 如果你只是想避免调试模式下的 panic，那么**不要**使用此函数。
        /// 你需要的是 [`wrapping_mul`]。
        ///
        /// # 安全性(Safety）
        ///
        /// 当出现以下情况时会导致未定义行为：
        #[doc = concat!("`self * rhs > ", stringify!($SelfT), "::MAX` or `self * rhs < ", stringify!($SelfT), "::MIN`,")]
        /// 也就是 [`checked_mul`] 会返回 `None` 的情况。
        ///
        /// [`unwrap_unchecked`]: option/enum.Option.html#method.unwrap_unchecked
        #[doc = concat!("[`checked_mul`]: ", stringify!($SelfT), "::checked_mul")]
        #[doc = concat!("[`wrapping_mul`]: ", stringify!($SelfT), "::wrapping_mul")]
        #[stable(feature = "unchecked_math", since = "1.79.0")]
        #[rustc_const_stable(feature = "unchecked_math", since = "1.79.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const unsafe fn unchecked_mul(self, rhs: Self) -> Self {
            assert_unsafe_precondition!(
                check_language_ub,
                concat!(stringify!($SelfT), "::unchecked_mul cannot overflow"),
                (
                    lhs: $SelfT = self,
                    rhs: $SelfT = rhs,
                ) => !lhs.overflowing_mul(rhs).1,
            );

            // SAFETY: 调用方必须保证乘法不会溢出，这是 `unchecked_mul` 的前置条件。
            unsafe {
                intrinsics::unchecked_mul(self, rhs)
            }
        }

        /// 检查型整数除法。计算 `self / rhs`，如果 `rhs == 0`
        /// 或除法结果溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN + 1).checked_div(-1), Some(", stringify!($Max), "));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.checked_div(-1), None);")]
        #[doc = concat!("assert_eq!((1", stringify!($SelfT), ").checked_div(0), None);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_checked_int_div", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_div(self, rhs: Self) -> Option<Self> {
            if intrinsics::unlikely(rhs == 0 || ((self == Self::MIN) && (rhs == -1))) {
                None
            } else {
                // SAFETY: 上面已经排除了除以零以及 `Self::MIN / -1` 的溢出情形。
                Some(unsafe { intrinsics::unchecked_div(self, rhs) })
            }
        }

        /// 严格整数除法。计算 `self / rhs`，如果发生溢出则 panic。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// 这种溢出唯一会发生在有符号类型的 `MIN / -1` 上（其中 `MIN`
        /// 是该类型的最小负值）；这等价于 `-MIN`，其正值过大而无法由该类型表示。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN + 1).strict_div(-1), ", stringify!($Max), ");")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = ", stringify!($SelfT), "::MIN.strict_div(-1);")]
        /// ```
        ///
        /// 以下代码会因除以零而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = (1", stringify!($SelfT), ").strict_div(0);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_div(self, rhs: Self) -> Self {
            let (a, b) = self.overflowing_div(rhs);
            if b { overflow_panic::div() } else { a }
        }

        /// 检查型欧几里得除法。计算 `self.div_euclid(rhs)`，
        /// 如果 `rhs == 0` 或除法结果溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN + 1).checked_div_euclid(-1), Some(", stringify!($Max), "));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.checked_div_euclid(-1), None);")]
        #[doc = concat!("assert_eq!((1", stringify!($SelfT), ").checked_div_euclid(0), None);")]
        /// ```
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_div_euclid(self, rhs: Self) -> Option<Self> {
            // 使用 `&` 有助于 LLVM 看出这与除法中的检查相同。
            if intrinsics::unlikely(rhs == 0 || ((self == Self::MIN) & (rhs == -1))) {
                None
            } else {
                Some(self.div_euclid(rhs))
            }
        }

        /// 严格欧几里得除法。计算 `self.div_euclid(rhs)`，如果发生溢出则 panic。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// 这种溢出唯一会发生在有符号类型的 `MIN / -1` 上（其中 `MIN`
        /// 是该类型的最小负值）；这等价于 `-MIN`，其正值过大而无法由该类型表示。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN + 1).strict_div_euclid(-1), ", stringify!($Max), ");")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = ", stringify!($SelfT), "::MIN.strict_div_euclid(-1);")]
        /// ```
        ///
        /// 以下代码会因除以零而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = (1", stringify!($SelfT), ").strict_div_euclid(0);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_div_euclid(self, rhs: Self) -> Self {
            let (a, b) = self.overflowing_div_euclid(rhs);
            if b { overflow_panic::div() } else { a }
        }

        /// 检查型无余数整数除法。计算 `self / rhs`，如果 `rhs == 0`、
        /// 除法结果溢出，或 `self % rhs != 0`，则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(exact_div)]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN + 1).checked_div_exact(-1), Some(", stringify!($Max), "));")]
        #[doc = concat!("assert_eq!((-5", stringify!($SelfT), ").checked_div_exact(2), None);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.checked_div_exact(-1), None);")]
        #[doc = concat!("assert_eq!((1", stringify!($SelfT), ").checked_div_exact(0), None);")]
        /// ```
        #[unstable(
            feature = "exact_div",
            issue = "139911",
        )]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_div_exact(self, rhs: Self) -> Option<Self> {
            if intrinsics::unlikely(rhs == 0 || ((self == Self::MIN) && (rhs == -1))) {
                None
            } else {
                // SAFETY: 上面已经排除了除以零以及 `Self::MIN / -1` 的溢出情形。
                unsafe {
                    if intrinsics::unlikely(intrinsics::unchecked_rem(self, rhs) != 0) {
                        None
                    } else {
                        Some(intrinsics::exact_div(self, rhs))
                    }
                }
            }
        }

        /// 无余数整数除法。计算 `self / rhs`，如果 `self % rhs != 0` 则返回 `None`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs == 0`，此函数会 panic。
        ///
        /// ## 溢出行为
        ///
        /// 发生溢出时，如果启用了溢出检查（调试模式下默认启用），此函数会 panic；
        /// 如果禁用了溢出检查（发布模式下默认禁用），则会回绕。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(exact_div)]
        #[doc = concat!("assert_eq!(64", stringify!($SelfT), ".div_exact(2), Some(32));")]
        #[doc = concat!("assert_eq!(64", stringify!($SelfT), ".div_exact(32), Some(2));")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN + 1).div_exact(-1), Some(", stringify!($Max), "));")]
        #[doc = concat!("assert_eq!(65", stringify!($SelfT), ".div_exact(2), None);")]
        /// ```
        /// ```should_panic
        /// #![feature(exact_div)]
        #[doc = concat!("let _ = 64", stringify!($SelfT),".div_exact(0);")]
        /// ```
        /// ```should_panic
        /// #![feature(exact_div)]
        #[doc = concat!("let _ = ", stringify!($SelfT), "::MIN.div_exact(-1);")]
        /// ```
        #[unstable(
            feature = "exact_div",
            issue = "139911",
        )]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[rustc_inherit_overflow_checks]
        pub const fn div_exact(self, rhs: Self) -> Option<Self> {
            if self % rhs != 0 {
                None
            } else {
                Some(self / rhs)
            }
        }

        /// 不检查的无余数整数除法。计算 `self / rhs`。
        ///
        /// # 安全性(Safety）
        ///
        /// 当 `rhs == 0`、`self % rhs != 0`，或者出现以下情况时会导致未定义行为：
        #[doc = concat!("`self == ", stringify!($SelfT), "::MIN && rhs == -1`,")]
        /// 也就是 [`checked_div_exact`](Self::checked_div_exact) 会返回 `None` 的情况。
        #[unstable(
            feature = "exact_div",
            issue = "139911",
        )]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const unsafe fn unchecked_div_exact(self, rhs: Self) -> Self {
            assert_unsafe_precondition!(
                check_language_ub,
                concat!(stringify!($SelfT), "::unchecked_div_exact cannot overflow, divide by zero, or leave a remainder"),
                (
                    lhs: $SelfT = self,
                    rhs: $SelfT = rhs,
                ) => rhs > 0 && lhs % rhs == 0 && (lhs != <$SelfT>::MIN || rhs != -1),
            );
            // SAFETY: 调用方必须满足与 `exact_div` 相同的前置条件：非零、无余数且不溢出。
            unsafe { intrinsics::exact_div(self, rhs) }
        }

        /// 检查型整数取余。计算 `self % rhs`，如果 `rhs == 0`
        /// 或除法结果溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".checked_rem(2), Some(1));")]
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".checked_rem(0), None);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.checked_rem(-1), None);")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_checked_int_div", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_rem(self, rhs: Self) -> Option<Self> {
            if intrinsics::unlikely(rhs == 0 || ((self == Self::MIN) && (rhs == -1))) {
                None
            } else {
                // SAFETY: 上面已经排除了除以零以及 `Self::MIN / -1` 的溢出情形。
                Some(unsafe { intrinsics::unchecked_rem(self, rhs) })
            }
        }

        /// 严格整数取余。计算 `self % rhs`，如果除法结果溢出则 panic。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// 这种溢出唯一会发生在有符号类型的 `MIN / -1` 对应的 `x % y` 上
        /// （其中 `MIN` 是最小负值）；由于实现细节，这种取余是无效的。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".strict_rem(2), 1);")]
        /// ```
        ///
        /// 以下代码会因除以零而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = 5", stringify!($SelfT), ".strict_rem(0);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = ", stringify!($SelfT), "::MIN.strict_rem(-1);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_rem(self, rhs: Self) -> Self {
            let (a, b) = self.overflowing_rem(rhs);
            if b { overflow_panic::rem() } else { a }
        }

        /// 检查型欧几里得取余。计算 `self.rem_euclid(rhs)`，
        /// 如果 `rhs == 0` 或除法结果溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".checked_rem_euclid(2), Some(1));")]
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".checked_rem_euclid(0), None);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.checked_rem_euclid(-1), None);")]
        /// ```
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_rem_euclid(self, rhs: Self) -> Option<Self> {
            // 使用 `&` 有助于 LLVM 看出这与除法中的检查相同。
            if intrinsics::unlikely(rhs == 0 || ((self == Self::MIN) & (rhs == -1))) {
                None
            } else {
                Some(self.rem_euclid(rhs))
            }
        }

        /// 严格欧几里得取余。计算 `self.rem_euclid(rhs)`，如果除法结果溢出则 panic。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// 这种溢出唯一会发生在有符号类型的 `MIN / -1` 对应的 `x % y` 上
        /// （其中 `MIN` 是最小负值）；由于实现细节，这种取余是无效的。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".strict_rem_euclid(2), 1);")]
        /// ```
        ///
        /// 以下代码会因除以零而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = 5", stringify!($SelfT), ".strict_rem_euclid(0);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = ", stringify!($SelfT), "::MIN.strict_rem_euclid(-1);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_rem_euclid(self, rhs: Self) -> Self {
            let (a, b) = self.overflowing_rem_euclid(rhs);
            if b { overflow_panic::rem() } else { a }
        }

        /// 检查型取负。计算 `-self`，如果 `self == MIN` 则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".checked_neg(), Some(-5));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.checked_neg(), None);")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_checked_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_neg(self) -> Option<Self> {
            let (a, b) = self.overflowing_neg();
            if intrinsics::unlikely(b) { None } else { Some(a) }
        }

        /// 不检查的取负。计算 `-self`，并假定不会发生溢出。
        ///
        /// # 安全性(Safety）
        ///
        /// 当出现以下情况时会导致未定义行为：
        #[doc = concat!("`self == ", stringify!($SelfT), "::MIN`,")]
        /// 也就是 [`checked_neg`] 会返回 `None` 的情况。
        ///
        #[doc = concat!("[`checked_neg`]: ", stringify!($SelfT), "::checked_neg")]
        #[stable(feature = "unchecked_neg", since = "1.93.0")]
        #[rustc_const_stable(feature = "unchecked_neg", since = "1.93.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const unsafe fn unchecked_neg(self) -> Self {
            assert_unsafe_precondition!(
                check_language_ub,
                concat!(stringify!($SelfT), "::unchecked_neg cannot overflow"),
                (
                    lhs: $SelfT = self,
                ) => !lhs.overflowing_neg().1,
            );

            // SAFETY: 调用方必须保证取负不会溢出，也就是 `self != MIN`。
            unsafe {
                intrinsics::unchecked_sub(0, self)
            }
        }

        /// 严格取负。计算 `-self`，如果 `self == MIN` 则 panic。
        ///
        /// # Panics
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".strict_neg(), -5);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = ", stringify!($SelfT), "::MIN.strict_neg();")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_neg(self) -> Self {
            let (a, b) = self.overflowing_neg();
            if b { overflow_panic::neg() } else { a }
        }

        /// 检查型左移。计算 `self << rhs`，如果 `rhs` 大于或等于 `self`
        /// 的位数则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0x1", stringify!($SelfT), ".checked_shl(4), Some(0x10));")]
        #[doc = concat!("assert_eq!(0x1", stringify!($SelfT), ".checked_shl(129), None);")]
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".checked_shl(", stringify!($BITS_MINUS_ONE), "), Some(0));")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_checked_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_shl(self, rhs: u32) -> Option<Self> {
            // 不使用 `overflowing_shl`，因为那是回绕移位。
            if rhs < Self::BITS {
                // SAFETY: 刚刚已经检查过右操作数在有效范围内。
                Some(unsafe { self.unchecked_shl(rhs) })
            } else {
                None
            }
        }

        /// 严格左移。计算 `self << rhs`，如果 `rhs` 大于或等于 `self`
        /// 的位数则 panic。
        ///
        /// # Panics
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0x1", stringify!($SelfT), ".strict_shl(4), 0x10);")]
        /// ```
        ///
        /// 以下代码会因移位位数越界而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = 0x1", stringify!($SelfT), ".strict_shl(129);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_shl(self, rhs: u32) -> Self {
            let (a, b) = self.overflowing_shl(rhs);
            if b { overflow_panic::shl() } else { a }
        }

        /// 不检查的左移。计算 `self << rhs`，并假定 `rhs` 小于 `self` 的位数。
        ///
        /// # 安全性(Safety）
        ///
        /// 如果 `rhs` 大于或等于 `self` 的位数，也就是 [`checked_shl`]
        /// 会返回 `None` 的情况，会导致未定义行为。
        ///
        #[doc = concat!("[`checked_shl`]: ", stringify!($SelfT), "::checked_shl")]
        #[stable(feature = "unchecked_shifts", since = "1.93.0")]
        #[rustc_const_stable(feature = "unchecked_shifts", since = "1.93.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const unsafe fn unchecked_shl(self, rhs: u32) -> Self {
            assert_unsafe_precondition!(
                check_language_ub,
                concat!(stringify!($SelfT), "::unchecked_shl cannot overflow"),
                (
                    rhs: u32 = rhs,
                ) => rhs < <$ActualT>::BITS,
            );

            // SAFETY: 调用方必须保证 `rhs < Self::BITS`，这是 `unchecked_shl` 的前置条件。
            unsafe {
                intrinsics::unchecked_shl(self, rhs)
            }
        }

        /// 无界左移。计算 `self << rhs`，不要求 `rhs` 预先限制在位宽范围内。
        ///
        /// 如果 `rhs` 大于或等于 `self` 的位数，整个值都会被移出，并返回 `0`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0x1_", stringify!($SelfT), ".unbounded_shl(4), 0x10);")]
        #[doc = concat!("assert_eq!(0x1_", stringify!($SelfT), ".unbounded_shl(129), 0);")]
        #[doc = concat!("assert_eq!(0b101_", stringify!($SelfT), ".unbounded_shl(0), 0b101);")]
        #[doc = concat!("assert_eq!(0b101_", stringify!($SelfT), ".unbounded_shl(1), 0b1010);")]
        #[doc = concat!("assert_eq!(0b101_", stringify!($SelfT), ".unbounded_shl(2), 0b10100);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".unbounded_shl(", stringify!($BITS), "), 0);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".unbounded_shl(1).unbounded_shl(", stringify!($BITS_MINUS_ONE), "), 0);")]
        #[doc = concat!("assert_eq!((-13_", stringify!($SelfT), ").unbounded_shl(", stringify!($BITS), "), 0);")]
        #[doc = concat!("assert_eq!((-13_", stringify!($SelfT), ").unbounded_shl(1).unbounded_shl(", stringify!($BITS_MINUS_ONE), "), 0);")]
        /// ```
        #[stable(feature = "unbounded_shifts", since = "1.87.0")]
        #[rustc_const_stable(feature = "unbounded_shifts", since = "1.87.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn unbounded_shl(self, rhs: u32) -> $SelfT{
            if rhs < Self::BITS {
                // SAFETY:
                // 上面刚刚检查过 `rhs` 在有效范围内。
                unsafe { self.unchecked_shl(rhs) }
            } else {
                0
            }
        }

        /// 精确左移。在可无损反向恢复的前提下计算 `self << rhs`。
        ///
        /// 如果任何会被移出的位不同于结果符号位，或 `rhs` >=
        #[doc = concat!("`", stringify!($SelfT), "::BITS`.")]
        /// 否则返回 `Some(self << rhs)`。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(exact_bitshifts)]
        ///
        #[doc = concat!("assert_eq!(0x1", stringify!($SelfT), ".shl_exact(4), Some(0x10));")]
        #[doc = concat!("assert_eq!(0x1", stringify!($SelfT), ".shl_exact(", stringify!($SelfT), "::BITS - 2), Some(1 << ", stringify!($SelfT), "::BITS - 2));")]
        #[doc = concat!("assert_eq!(0x1", stringify!($SelfT), ".shl_exact(", stringify!($SelfT), "::BITS - 1), None);")]
        #[doc = concat!("assert_eq!((-0x2", stringify!($SelfT), ").shl_exact(", stringify!($SelfT), "::BITS - 2), Some(-0x2 << ", stringify!($SelfT), "::BITS - 2));")]
        #[doc = concat!("assert_eq!((-0x2", stringify!($SelfT), ").shl_exact(", stringify!($SelfT), "::BITS - 1), None);")]
        /// ```
        #[unstable(feature = "exact_bitshifts", issue = "144336")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn shl_exact(self, rhs: u32) -> Option<$SelfT> {
            if rhs < self.leading_zeros() || rhs < self.leading_ones() {
                // SAFETY: 上面已经检查过 `rhs`。
                Some(unsafe { self.unchecked_shl(rhs) })
            } else {
                None
            }
        }

        /// 不检查的精确左移。计算 `self << rhs`，并假定该操作可无损反向恢复，
        /// 且 `rhs` 不会大于
        #[doc = concat!("`", stringify!($SelfT), "::BITS`.")]
        ///
        /// # 安全性(Safety）
        ///
        /// 当 `rhs >= self.leading_zeros() && rhs >= self.leading_ones()`，
        /// 也就是
        #[doc = concat!("[`", stringify!($SelfT), "::shl_exact`]")]
        /// 会返回 `None` 的情况，会导致未定义行为。
        #[unstable(feature = "exact_bitshifts", issue = "144336")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const unsafe fn unchecked_shl_exact(self, rhs: u32) -> $SelfT {
            assert_unsafe_precondition!(
                check_library_ub,
                concat!(stringify!($SelfT), "::unchecked_shl_exact cannot shift out bits that would change the value of the first bit"),
                (
                    zeros: u32 = self.leading_zeros(),
                    ones: u32 = self.leading_ones(),
                    rhs: u32 = rhs,
                ) => rhs < zeros || rhs < ones,
            );

            // SAFETY: 调用方必须保证左移不会移出会改变符号扩展语义的位。
            unsafe { self.unchecked_shl(rhs) }
        }

        /// 检查型右移。计算 `self >> rhs`，如果 `rhs` 大于或等于 `self`
        /// 的位数则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".checked_shr(4), Some(0x1));")]
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".checked_shr(128), None);")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_checked_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_shr(self, rhs: u32) -> Option<Self> {
            // 不使用 `overflowing_shr`，因为那是回绕移位。
            if rhs < Self::BITS {
                // SAFETY: 刚刚已经检查过右操作数在有效范围内。
                Some(unsafe { self.unchecked_shr(rhs) })
            } else {
                None
            }
        }

        /// 严格右移。计算 `self >> rhs`，如果 `rhs` 大于或等于 `self`
        /// 的位数则 panic。
        ///
        /// # Panics
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".strict_shr(4), 0x1);")]
        /// ```
        ///
        /// 以下代码会因移位位数越界而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = 0x10", stringify!($SelfT), ".strict_shr(128);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_shr(self, rhs: u32) -> Self {
            let (a, b) = self.overflowing_shr(rhs);
            if b { overflow_panic::shr() } else { a }
        }

        /// 不检查的右移。计算 `self >> rhs`，并假定 `rhs` 小于 `self` 的位数。
        ///
        /// # 安全性(Safety）
        ///
        /// 如果 `rhs` 大于或等于 `self` 的位数，也就是 [`checked_shr`]
        /// 会返回 `None` 的情况，会导致未定义行为。
        ///
        #[doc = concat!("[`checked_shr`]: ", stringify!($SelfT), "::checked_shr")]
        #[stable(feature = "unchecked_shifts", since = "1.93.0")]
        #[rustc_const_stable(feature = "unchecked_shifts", since = "1.93.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const unsafe fn unchecked_shr(self, rhs: u32) -> Self {
            assert_unsafe_precondition!(
                check_language_ub,
                concat!(stringify!($SelfT), "::unchecked_shr cannot overflow"),
                (
                    rhs: u32 = rhs,
                ) => rhs < <$ActualT>::BITS,
            );

            // SAFETY: 调用方必须保证 `rhs < Self::BITS`，这是 `unchecked_shr` 的前置条件。
            unsafe {
                intrinsics::unchecked_shr(self, rhs)
            }
        }

        /// 无界右移。计算 `self >> rhs`，不要求 `rhs` 预先限制在位宽范围内。
        ///
        /// 如果 `rhs` 大于或等于 `self` 的位数，整个值都会被移出；
        /// 正数会得到 `0`，负数会得到 `-1`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0x10_", stringify!($SelfT), ".unbounded_shr(4), 0x1);")]
        #[doc = concat!("assert_eq!(0x10_", stringify!($SelfT), ".unbounded_shr(129), 0);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.unbounded_shr(129), -1);")]
        #[doc = concat!("assert_eq!(0b1010_", stringify!($SelfT), ".unbounded_shr(0), 0b1010);")]
        #[doc = concat!("assert_eq!(0b1010_", stringify!($SelfT), ".unbounded_shr(1), 0b101);")]
        #[doc = concat!("assert_eq!(0b1010_", stringify!($SelfT), ".unbounded_shr(2), 0b10);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".unbounded_shr(", stringify!($BITS), "), 0);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".unbounded_shr(1).unbounded_shr(", stringify!($BITS_MINUS_ONE), "), 0);")]
        #[doc = concat!("assert_eq!((-13_", stringify!($SelfT), ").unbounded_shr(", stringify!($BITS), "), -1);")]
        #[doc = concat!("assert_eq!((-13_", stringify!($SelfT), ").unbounded_shr(1).unbounded_shr(", stringify!($BITS_MINUS_ONE), "), -1);")]
        /// ```
        #[stable(feature = "unbounded_shifts", since = "1.87.0")]
        #[rustc_const_stable(feature = "unbounded_shifts", since = "1.87.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn unbounded_shr(self, rhs: u32) -> $SelfT{
            if rhs < Self::BITS {
                // SAFETY:
                // 上面刚刚检查过 `rhs` 在有效范围内。
                unsafe { self.unchecked_shr(rhs) }
            } else {
                // 对有符号整数来说，移位 `Self::BITS - 1` 已足够，因为每个移入位都会复制符号位。

                // SAFETY:
                // `Self::BITS - 1` 保证小于 `Self::BITS`。
                unsafe { self.unchecked_shr(Self::BITS - 1) }
            }
        }

        /// 精确右移。在可无损反向恢复的前提下计算 `self >> rhs`。
        ///
        /// 如果任何非零位会被移出，或 `rhs` >=
        #[doc = concat!("`", stringify!($SelfT), "::BITS`.")]
        /// 否则返回 `Some(self >> rhs)`。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(exact_bitshifts)]
        ///
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".shr_exact(4), Some(0x1));")]
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".shr_exact(5), None);")]
        /// ```
        #[unstable(feature = "exact_bitshifts", issue = "144336")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn shr_exact(self, rhs: u32) -> Option<$SelfT> {
            if rhs <= self.trailing_zeros() && rhs < <$SelfT>::BITS {
                // SAFETY: 上面已经检查过 `rhs`。
                Some(unsafe { self.unchecked_shr(rhs) })
            } else {
                None
            }
        }

        /// 不检查的精确右移。计算 `self >> rhs`，并假定该操作可无损反向恢复，
        /// 且 `rhs` 不会大于
        #[doc = concat!("`", stringify!($SelfT), "::BITS`.")]
        ///
        /// # 安全性(Safety）
        ///
        /// 当 `rhs > self.trailing_zeros() || rhs >=
        #[doc = concat!(stringify!($SelfT), "::BITS`")]
        /// 也就是
        #[doc = concat!("[`", stringify!($SelfT), "::shr_exact`]")]
        /// 会返回 `None` 的情况，会导致未定义行为。
        #[unstable(feature = "exact_bitshifts", issue = "144336")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const unsafe fn unchecked_shr_exact(self, rhs: u32) -> $SelfT {
            assert_unsafe_precondition!(
                check_library_ub,
                concat!(stringify!($SelfT), "::unchecked_shr_exact cannot shift out non-zero bits"),
                (
                    zeros: u32 = self.trailing_zeros(),
                    bits: u32 =  <$SelfT>::BITS,
                    rhs: u32 = rhs,
                ) => rhs <= zeros && rhs < bits,
            );

            // SAFETY: 调用方必须保证右移不会移出任何非零位且移位位数在范围内。
            unsafe { self.unchecked_shr(rhs) }
        }

        /// 检查型绝对值。计算 `self.abs()`，如果 `self == MIN` 则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!((-5", stringify!($SelfT), ").checked_abs(), Some(5));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.checked_abs(), None);")]
        /// ```
        #[stable(feature = "no_panic_abs", since = "1.13.0")]
        #[rustc_const_stable(feature = "const_checked_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_abs(self) -> Option<Self> {
            if self.is_negative() {
                self.checked_neg()
            } else {
                Some(self)
            }
        }

        /// 严格绝对值。计算 `self.abs()`，如果 `self == MIN` 则 panic。
        ///
        /// # Panics
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!((-5", stringify!($SelfT), ").strict_abs(), 5);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = ", stringify!($SelfT), "::MIN.strict_abs();")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_abs(self) -> Self {
            if self.is_negative() {
                self.strict_neg()
            } else {
                self
            }
        }

        /// 检查型乘方。计算 `self.pow(exp)`，如果发生溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(8", stringify!($SelfT), ".checked_pow(2), Some(64));")]
        #[doc = concat!("assert_eq!(0_", stringify!($SelfT), ".checked_pow(0), Some(1));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.checked_pow(2), None);")]
        /// ```

        #[stable(feature = "no_panic_pow", since = "1.34.0")]
        #[rustc_const_stable(feature = "const_int_pow", since = "1.50.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_pow(self, mut exp: u32) -> Option<Self> {
            if exp == 0 {
                return Some(1);
            }
            let mut base = self;
            let mut acc: Self = 1;

            loop {
                if (exp & 1) == 1 {
                    acc = try_opt!(acc.checked_mul(base));
                    // 因为 exp != 0，最终 exp 必须为 1。
                    if exp == 1 {
                        return Some(acc);
                    }
                }
                exp /= 2;
                base = try_opt!(base.checked_mul(base));
            }
        }

        /// 严格乘方。计算 `self.pow(exp)`，如果发生溢出则 panic。
        ///
        /// # Panics
        ///
        /// ## 溢出行为
        ///
        /// 无论是否启用溢出检查，此函数都会在溢出时 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(8", stringify!($SelfT), ".strict_pow(2), 64);")]
        #[doc = concat!("assert_eq!(0_", stringify!($SelfT), ".strict_pow(0), 1);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = ", stringify!($SelfT), "::MAX.strict_pow(2);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_pow(self, mut exp: u32) -> Self {
            if exp == 0 {
                return 1;
            }
            let mut base = self;
            let mut acc: Self = 1;

            loop {
                if (exp & 1) == 1 {
                    acc = acc.strict_mul(base);
                    // 因为 exp != 0，最终 exp 必须为 1。
                    if exp == 1 {
                        return acc;
                    }
                }
                exp /= 2;
                base = base.strict_mul(base);
            }
        }

        /// 返回该数的平方根，向下取整。
        ///
        /// 如果 `self` 为负数，则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(10", stringify!($SelfT), ".checked_isqrt(), Some(3));")]
        /// ```
        #[stable(feature = "isqrt", since = "1.84.0")]
        #[rustc_const_stable(feature = "isqrt", since = "1.84.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_isqrt(self) -> Option<Self> {
            if self < 0 {
                None
            } else {
                // SAFETY: 此 `else` 分支中输入已知为非负数。
                let result = unsafe {
                    crate::num::int_sqrt::$ActualT(self as $ActualT) as $SelfT
                };

                // 告诉优化器输出范围。如果测试 `core` 时崩溃但没有 panic 消息，
                // 且某个 `num::int_sqrt::i*` 测试失败，说明你的编辑使这些断言变为假。
                //
                // SAFETY: 整数平方根是单调不减函数，输入增大不会导致输出减小。
                // 因此，非负有符号整数的输入范围受 `[0, <$ActualT>::MAX]` 限制时，
                // sqrt(n) 的范围也会受 `[sqrt(0), sqrt(<$ActualT>::MAX)]` 限制。
                unsafe {
                    // SAFETY: `<$ActualT>::MAX` 是非负数。
                    const MAX_RESULT: $SelfT = unsafe {
                        crate::num::int_sqrt::$ActualT(<$ActualT>::MAX) as $SelfT
                    };

                    crate::hint::assert_unchecked(result >= 0);
                    crate::hint::assert_unchecked(result <= MAX_RESULT);
                }

                Some(result)
            }
        }

        /// 饱和整数加法。计算 `self + rhs`，在数值边界处饱和而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".saturating_add(1), 101);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.saturating_add(100), ", stringify!($SelfT), "::MAX);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.saturating_add(-1), ", stringify!($SelfT), "::MIN);")]
        /// ```

        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_saturating_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn saturating_add(self, rhs: Self) -> Self {
            intrinsics::saturating_add(self, rhs)
        }

        /// 与无符号整数相加的饱和加法。计算 `self + rhs`，
        /// 在数值边界处饱和而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".saturating_add_unsigned(2), 3);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.saturating_add_unsigned(100), ", stringify!($SelfT), "::MAX);")]
        /// ```
        #[stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn saturating_add_unsigned(self, rhs: $UnsignedT) -> Self {
            // 溢出只可能发生在上界。
            // 这里不能使用 `unwrap_or`，因为它不是 `const`。
            match self.checked_add_unsigned(rhs) {
                Some(x) => x,
                None => Self::MAX,
            }
        }

        /// 饱和整数减法。计算 `self - rhs`，在数值边界处饱和而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".saturating_sub(127), -27);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.saturating_sub(100), ", stringify!($SelfT), "::MIN);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.saturating_sub(-1), ", stringify!($SelfT), "::MAX);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_saturating_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn saturating_sub(self, rhs: Self) -> Self {
            intrinsics::saturating_sub(self, rhs)
        }

        /// 与无符号整数相减的饱和减法。计算 `self - rhs`，
        /// 在数值边界处饱和而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".saturating_sub_unsigned(127), -27);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.saturating_sub_unsigned(100), ", stringify!($SelfT), "::MIN);")]
        /// ```
        #[stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn saturating_sub_unsigned(self, rhs: $UnsignedT) -> Self {
            // 溢出只可能发生在下界。
            // 这里不能使用 `unwrap_or`，因为它不是 `const`。
            match self.checked_sub_unsigned(rhs) {
                Some(x) => x,
                None => Self::MIN,
            }
        }

        /// 饱和整数取负。计算 `-self`，如果 `self == MIN` 则返回 `MAX`
        /// 而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".saturating_neg(), -100);")]
        #[doc = concat!("assert_eq!((-100", stringify!($SelfT), ").saturating_neg(), 100);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.saturating_neg(), ", stringify!($SelfT), "::MAX);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.saturating_neg(), ", stringify!($SelfT), "::MIN + 1);")]
        /// ```

        #[stable(feature = "saturating_neg", since = "1.45.0")]
        #[rustc_const_stable(feature = "const_saturating_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn saturating_neg(self) -> Self {
            intrinsics::saturating_sub(0, self)
        }

        /// 饱和绝对值。计算 `self.abs()`，如果 `self == MIN` 则返回 `MAX`
        /// 而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".saturating_abs(), 100);")]
        #[doc = concat!("assert_eq!((-100", stringify!($SelfT), ").saturating_abs(), 100);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.saturating_abs(), ", stringify!($SelfT), "::MAX);")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN + 1).saturating_abs(), ", stringify!($SelfT), "::MAX);")]
        /// ```

        #[stable(feature = "saturating_neg", since = "1.45.0")]
        #[rustc_const_stable(feature = "const_saturating_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn saturating_abs(self) -> Self {
            if self.is_negative() {
                self.saturating_neg()
            } else {
                self
            }
        }

        /// 饱和整数乘法。计算 `self * rhs`，在数值边界处饱和而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(10", stringify!($SelfT), ".saturating_mul(12), 120);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.saturating_mul(10), ", stringify!($SelfT), "::MAX);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.saturating_mul(10), ", stringify!($SelfT), "::MIN);")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_saturating_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn saturating_mul(self, rhs: Self) -> Self {
            match self.checked_mul(rhs) {
                Some(x) => x,
                None => if (self < 0) == (rhs < 0) {
                    Self::MAX
                } else {
                    Self::MIN
                }
            }
        }

        /// 饱和整数除法。计算 `self / rhs`，在数值边界处饱和而不是溢出。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".saturating_div(2), 2);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.saturating_div(-1), ", stringify!($SelfT), "::MIN + 1);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.saturating_div(-1), ", stringify!($SelfT), "::MAX);")]
        ///
        /// ```
        #[stable(feature = "saturating_div", since = "1.58.0")]
        #[rustc_const_stable(feature = "saturating_div", since = "1.58.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn saturating_div(self, rhs: Self) -> Self {
            match self.overflowing_div(rhs) {
                (result, false) => result,
                (_result, true) => Self::MAX, // MIN / -1 是唯一可能的饱和溢出情形。
            }
        }

        /// 饱和整数乘方。计算 `self.pow(exp)`，在数值边界处饱和而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!((-4", stringify!($SelfT), ").saturating_pow(3), -64);")]
        #[doc = concat!("assert_eq!(0_", stringify!($SelfT), ".saturating_pow(0), 1);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.saturating_pow(2), ", stringify!($SelfT), "::MAX);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.saturating_pow(3), ", stringify!($SelfT), "::MIN);")]
        /// ```
        #[stable(feature = "no_panic_pow", since = "1.34.0")]
        #[rustc_const_stable(feature = "const_int_pow", since = "1.50.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn saturating_pow(self, exp: u32) -> Self {
            match self.checked_pow(exp) {
                Some(x) => x,
                None if self < 0 && exp % 2 == 1 => Self::MIN,
                None => Self::MAX,
            }
        }

        /// 回绕（模）加法。计算 `self + rhs`，并在类型边界处回绕。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_add(27), 127);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.wrapping_add(2), ", stringify!($SelfT), "::MIN + 1);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_add(self, rhs: Self) -> Self {
            intrinsics::wrapping_add(self, rhs)
        }

        /// 与无符号整数相加的回绕（模）加法。计算 `self + rhs`，
        /// 并在类型边界处回绕。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_add_unsigned(27), 127);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.wrapping_add_unsigned(2), ", stringify!($SelfT), "::MIN + 1);")]
        /// ```
        #[stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_add_unsigned(self, rhs: $UnsignedT) -> Self {
            self.wrapping_add(rhs as Self)
        }

        /// 回绕（模）减法。计算 `self - rhs`，并在类型边界处回绕。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".wrapping_sub(127), -127);")]
        #[doc = concat!("assert_eq!((-2", stringify!($SelfT), ").wrapping_sub(", stringify!($SelfT), "::MAX), ", stringify!($SelfT), "::MAX);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_sub(self, rhs: Self) -> Self {
            intrinsics::wrapping_sub(self, rhs)
        }

        /// 与无符号整数相减的回绕（模）减法。计算 `self - rhs`，
        /// 并在类型边界处回绕。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".wrapping_sub_unsigned(127), -127);")]
        #[doc = concat!("assert_eq!((-2", stringify!($SelfT), ").wrapping_sub_unsigned(", stringify!($UnsignedT), "::MAX), -1);")]
        /// ```
        #[stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_sub_unsigned(self, rhs: $UnsignedT) -> Self {
            self.wrapping_sub(rhs as Self)
        }

        /// 回绕（模）乘法。计算 `self * rhs`，并在类型边界处回绕。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(10", stringify!($SelfT), ".wrapping_mul(12), 120);")]
        /// assert_eq!(11i8.wrapping_mul(12), -124);
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_mul(self, rhs: Self) -> Self {
            intrinsics::wrapping_mul(self, rhs)
        }

        /// 回绕（模）除法。计算 `self / rhs`，并在类型边界处回绕。
        ///
        /// 唯一会发生这种回绕的情况是在有符号类型上计算 `MIN / -1`（其中
        /// `MIN` 是该类型的最小负值）；这等价于 `-MIN`，一个过大而无法
        /// 用该类型表示的正值。在这种情况下，此函数会返回 `MIN` 本身。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数将会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_div(10), 10);")]
        /// assert_eq!((-128i8).wrapping_div(-1), -128);
        /// ```
        #[stable(feature = "num_wrapping", since = "1.2.0")]
        #[rustc_const_stable(feature = "const_wrapping_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn wrapping_div(self, rhs: Self) -> Self {
            self.overflowing_div(rhs).0
        }

        /// 回绕欧几里得除法。计算 `self.div_euclid(rhs)`，
        /// 并在类型边界处回绕。
        ///
        /// 只有在有符号类型上计算 `MIN / -1`（其中 `MIN` 是该类型的最小负值）时
        /// 才会发生回绕。这等价于 `-MIN`，一个过大而无法用该类型表示的正值。
        /// 在这种情况下，此方法会返回 `MIN` 本身。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数将会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_div_euclid(10), 10);")]
        /// assert_eq!((-128i8).wrapping_div_euclid(-1), -128);
        /// ```
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn wrapping_div_euclid(self, rhs: Self) -> Self {
            self.overflowing_div_euclid(rhs).0
        }

        /// 回绕（模）取余。计算 `self % rhs`，并在类型边界处回绕。
        ///
        /// 从数学上讲，这种回绕实际上永远不会发生；实现细节会使有符号类型上
        /// `MIN / -1`（其中 `MIN` 是最小负值）对应的 `x % y` 无效。
        /// 在这种情况下，此函数会返回 `0`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数将会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_rem(10), 0);")]
        /// assert_eq!((-128i8).wrapping_rem(-1), 0);
        /// ```
        #[stable(feature = "num_wrapping", since = "1.2.0")]
        #[rustc_const_stable(feature = "const_wrapping_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn wrapping_rem(self, rhs: Self) -> Self {
            self.overflowing_rem(rhs).0
        }

        /// 回绕欧几里得取余。计算 `self.rem_euclid(rhs)`，并在类型边界处回绕。
        ///
        /// 只有在有符号类型上计算 `MIN % -1`（其中 `MIN` 是该类型的最小负值）时
        /// 才会发生回绕。在这种情况下，此方法会返回 0。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数将会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_rem_euclid(10), 0);")]
        /// assert_eq!((-128i8).wrapping_rem_euclid(-1), 0);
        /// ```
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn wrapping_rem_euclid(self, rhs: Self) -> Self {
            self.overflowing_rem_euclid(rhs).0
        }

        /// 回绕（模）取负。计算 `-self`，并在类型边界处回绕。
        ///
        /// 唯一会发生这种回绕的情况是在有符号类型上对 `MIN` 取负（其中 `MIN`
        /// 是该类型的最小负值）；这是一个过大而无法用该类型表示的正值。
        /// 在这种情况下，此函数会返回 `MIN` 本身。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_neg(), -100);")]
        #[doc = concat!("assert_eq!((-100", stringify!($SelfT), ").wrapping_neg(), 100);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.wrapping_neg(), ", stringify!($SelfT), "::MIN);")]
        /// ```
        #[stable(feature = "num_wrapping", since = "1.2.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_neg(self) -> Self {
            (0 as $SelfT).wrapping_sub(self)
        }

        /// 不会 panic 的按位左移；产生 `self << mask(rhs)`，其中 `mask`
        /// 会移除 `rhs` 中会导致移位量超过类型位宽的高位。
        ///
        /// 请注意，与整数上大多数其他 `wrapping_*` 方法不同，这*不会*得到
        /// 先用无限精度执行移位、再按需截断时的相同结果。其行为与许多处理器
        /// 上移位指令的行为一致，也与禁用溢出检查时 `<<` 运算符的行为一致，
        /// 但从数值角度看比较反常。请考虑改用行为更自然的 [`Self::unbounded_shl`]。
        ///
        /// 注意，这与循环左移*不同*；回绕左移的右操作数会被限制在该类型的范围内，
        /// 而不是把从左操作数移出的位送回另一端。所有基本整数类型都实现了
        /// [`rotate_left`](Self::rotate_left) 函数，它可能才是你需要的操作。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!((-1_", stringify!($SelfT), ").wrapping_shl(7), -128);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".wrapping_shl(", stringify!($BITS), "), 42);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".wrapping_shl(1).wrapping_shl(", stringify!($BITS_MINUS_ONE), "), 0);")]
        #[doc = concat!("assert_eq!((-1_", stringify!($SelfT), ").wrapping_shl(128), -1);")]
        #[doc = concat!("assert_eq!(5_", stringify!($SelfT), ".wrapping_shl(1025), 10);")]
        /// ```
        #[stable(feature = "num_wrapping", since = "1.2.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_shl(self, rhs: u32) -> Self {
            // SAFETY: 用类型位宽进行掩码后，移位量一定小于 Self::BITS，
            // 因而满足 `unchecked_shl` 的前置条件，不会越界移位。
            unsafe {
                self.unchecked_shl(rhs & (Self::BITS - 1))
            }
        }

        /// 不会 panic 的按位右移；产生 `self >> mask(rhs)`，其中 `mask`
        /// 会移除 `rhs` 中会导致移位量超过类型位宽的高位。
        ///
        /// 请注意，与整数上大多数其他 `wrapping_*` 方法不同，这*不会*得到
        /// 先用无限精度执行移位、再按需截断时的相同结果。其行为与许多处理器
        /// 上移位指令的行为一致，也与禁用溢出检查时 `>>` 运算符的行为一致，
        /// 但从数值角度看比较反常。请考虑改用行为更自然的 [`Self::unbounded_shr`]。
        ///
        /// 注意，这与循环右移*不同*；回绕右移的右操作数会被限制在该类型的范围内，
        /// 而不是把从左操作数移出的位送回另一端。所有基本整数类型都实现了
        /// [`rotate_right`](Self::rotate_right) 函数，它可能才是你需要的操作。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!((-128_", stringify!($SelfT), ").wrapping_shr(7), -1);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".wrapping_shr(", stringify!($BITS), "), 42);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".wrapping_shr(1).wrapping_shr(", stringify!($BITS_MINUS_ONE), "), 0);")]
        /// assert_eq!((-128_i16).wrapping_shr(64), -128);
        #[doc = concat!("assert_eq!(10_", stringify!($SelfT), ".wrapping_shr(1025), 5);")]
        /// ```
        #[stable(feature = "num_wrapping", since = "1.2.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_shr(self, rhs: u32) -> Self {
            // SAFETY: 用类型位宽进行掩码后，移位量一定小于 Self::BITS，
            // 因而满足 `unchecked_shr` 的前置条件，不会越界移位。
            unsafe {
                self.unchecked_shr(rhs & (Self::BITS - 1))
            }
        }

        /// 回绕（模）绝对值。计算 `self.abs()`，并在类型边界处回绕。
        ///
        /// 唯一会发生这种回绕的情况是取得该类型最小负值的绝对值；这是一个
        /// 过大而无法用该类型表示的正值。在这种情况下，此函数会返回 `MIN` 本身。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_abs(), 100);")]
        #[doc = concat!("assert_eq!((-100", stringify!($SelfT), ").wrapping_abs(), 100);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.wrapping_abs(), ", stringify!($SelfT), "::MIN);")]
        /// assert_eq!((-128i8).wrapping_abs() as u8, 128);
        /// ```
        #[stable(feature = "no_panic_abs", since = "1.13.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[allow(unused_attributes)]
        #[inline]
        pub const fn wrapping_abs(self) -> Self {
             if self.is_negative() {
                 self.wrapping_neg()
             } else {
                 self
             }
        }

        /// 计算 `self` 的绝对值，不会发生任何回绕或 panic。
        ///
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".unsigned_abs(), 100", stringify!($UnsignedT), ");")]
        #[doc = concat!("assert_eq!((-100", stringify!($SelfT), ").unsigned_abs(), 100", stringify!($UnsignedT), ");")]
        /// assert_eq!((-128i8).unsigned_abs(), 128u8);
        /// ```
        #[stable(feature = "unsigned_abs", since = "1.51.0")]
        #[rustc_const_stable(feature = "unsigned_abs", since = "1.51.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn unsigned_abs(self) -> $UnsignedT {
             self.wrapping_abs() as $UnsignedT
        }

        /// 回绕（模）乘方。计算 `self.pow(exp)`，并在类型边界处回绕。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(3", stringify!($SelfT), ".wrapping_pow(4), 81);")]
        /// assert_eq!(3i8.wrapping_pow(5), -13);
        /// assert_eq!(3i8.wrapping_pow(6), -39);
        #[doc = concat!("assert_eq!(0_", stringify!($SelfT), ".wrapping_pow(0), 1);")]
        /// ```
        #[stable(feature = "no_panic_pow", since = "1.34.0")]
        #[rustc_const_stable(feature = "const_int_pow", since = "1.50.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn wrapping_pow(self, mut exp: u32) -> Self {
            if exp == 0 {
                return 1;
            }
            let mut base = self;
            let mut acc: Self = 1;

            if intrinsics::is_val_statically_known(exp) {
                while exp > 1 {
                    if (exp & 1) == 1 {
                        acc = acc.wrapping_mul(base);
                    }
                    exp /= 2;
                    base = base.wrapping_mul(base);
                }

                // 因为 exp != 0，最终 exp 必定为 1。
                // 单独处理指数的最后一位，因为之后没有必要再对底数平方。
                acc.wrapping_mul(base)
            } else {
                // 当指数在编译期未知时，这比上面的分支更快。常量指数场景不能
                // 使用同一段代码，因为 LLVM 目前无法展开这个循环。
                loop {
                    if (exp & 1) == 1 {
                        acc = acc.wrapping_mul(base);
                        // 因为 exp != 0，最终 exp 必定为 1。
                        if exp == 1 {
                            return acc;
                        }
                    }
                    exp /= 2;
                    base = base.wrapping_mul(base);
                }
            }
        }

        /// 计算 `self` + `rhs`。
        ///
        /// 返回加法结果以及一个表示是否会发生算术溢出的布尔值。
        /// 如果会发生溢出，则返回回绕后的值。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".overflowing_add(2), (7, false));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.overflowing_add(1), (", stringify!($SelfT), "::MIN, true));")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn overflowing_add(self, rhs: Self) -> (Self, bool) {
            let (a, b) = intrinsics::add_with_overflow(self as $ActualT, rhs as $ActualT);
            (a as Self, b)
        }

        /// 计算 `self` + `rhs` + `carry` 并检查溢出。
        ///
        /// 对两个整数操作数和一个进位输入位执行“三元加法”，并返回和以及一个
        /// 表示是否会发生算术溢出的布尔值。发生溢出时返回回绕后的值。
        ///
        /// 这允许把多次加法串接起来形成更宽的加法，可用于大数加法。
        /// 此方法只应当用于最高有效字；对于较低有效字，应当使用无符号方法
        #[doc = concat!("[`", stringify!($UnsignedT), "::carrying_add`]")]
        /// 。
        ///
        /// 此方法返回的输出布尔值*不是*进位标志，也*不应*加到更高有效字上。
        ///
        /// 如果输入进位为 false，此方法等价于 [`overflowing_add`](Self::overflowing_add)。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(bigint_helper_methods)]
        /// // 只有最高有效字是带符号的。
        /// //
        #[doc = concat!("//   10  MAX    (a = 10 × 2^", stringify!($BITS), " + 2^", stringify!($BITS), " - 1)")]
        #[doc = concat!("// + -5    9    (b = -5 × 2^", stringify!($BITS), " + 9)")]
        /// // ---------
        #[doc = concat!("//    6    8    (sum = 6 × 2^", stringify!($BITS), " + 8)")]
        ///
        #[doc = concat!("let (a1, a0): (", stringify!($SelfT), ", ", stringify!($UnsignedT), ") = (10, ", stringify!($UnsignedT), "::MAX);")]
        #[doc = concat!("let (b1, b0): (", stringify!($SelfT), ", ", stringify!($UnsignedT), ") = (-5, 9);")]
        /// let carry0 = false;
        ///
        #[doc = concat!("// ", stringify!($UnsignedT), "::carrying_add for the less significant words")]
        /// let (sum0, carry1) = a0.carrying_add(b0, carry0);
        /// assert_eq!(carry1, true);
        ///
        #[doc = concat!("// ", stringify!($SelfT), "::carrying_add for the most significant word")]
        /// let (sum1, overflow) = a1.carrying_add(b1, carry1);
        /// assert_eq!(overflow, false);
        ///
        /// assert_eq!((sum1, sum0), (6, 8));
        /// ```
        #[unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn carrying_add(self, rhs: Self, carry: bool) -> (Self, bool) {
            // 注意：长期来看这里应该通过 intrinsic 实现。
            // 注意：不需要中间溢出（https://github.com/rust-lang/rust/issues/85532#issuecomment-1032214946）。
            let (a, b) = self.overflowing_add(rhs);
            let (c, d) = a.overflowing_add(carry as $SelfT);
            (c, b != d)
        }

        /// 计算 `self` + `rhs`，其中 `rhs` 是无符号整数。
        ///
        /// 返回加法结果以及一个表示是否会发生算术溢出的布尔值。
        /// 如果会发生溢出，则返回回绕后的值。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".overflowing_add_unsigned(2), (3, false));")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN).overflowing_add_unsigned(", stringify!($UnsignedT), "::MAX), (", stringify!($SelfT), "::MAX, false));")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).overflowing_add_unsigned(3), (", stringify!($SelfT), "::MIN, true));")]
        /// ```
        #[stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn overflowing_add_unsigned(self, rhs: $UnsignedT) -> (Self, bool) {
            let rhs = rhs as Self;
            let (res, overflowed) = self.overflowing_add(rhs);
            (res, overflowed ^ (rhs < 0))
        }

        /// 计算 `self` - `rhs`。
        ///
        /// 返回减法结果以及一个表示是否会发生算术溢出的布尔值。
        /// 如果会发生溢出，则返回回绕后的值。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".overflowing_sub(2), (3, false));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.overflowing_sub(1), (", stringify!($SelfT), "::MAX, true));")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn overflowing_sub(self, rhs: Self) -> (Self, bool) {
            let (a, b) = intrinsics::sub_with_overflow(self as $ActualT, rhs as $ActualT);
            (a as Self, b)
        }

        /// 计算 `self` &minus; `rhs` &minus; `borrow` 并检查溢出。
        ///
        /// 从 `self` 同时减去一个整数操作数和一个借位输入位，以此执行
        /// “三元减法”，并返回差值以及一个表示是否会发生算术溢出的布尔值。
        /// 发生溢出时返回回绕后的值。
        ///
        /// 这允许把多次减法串接起来形成更宽的减法，可用于大数减法。
        /// 此方法只应当用于最高有效字；对于较低有效字，应当使用无符号方法
        #[doc = concat!("[`", stringify!($UnsignedT), "::borrowing_sub`]")]
        /// 。
        ///
        /// 此方法返回的输出布尔值*不是*借位标志，也*不应*从更高有效字中减去。
        ///
        /// 如果输入借位为 false，此方法等价于 [`overflowing_sub`](Self::overflowing_sub)。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(bigint_helper_methods)]
        /// // 只有最高有效字是带符号的。
        /// //
        #[doc = concat!("//    6    8    (a = 6 × 2^", stringify!($BITS), " + 8)")]
        #[doc = concat!("// - -5    9    (b = -5 × 2^", stringify!($BITS), " + 9)")]
        /// // ---------
        #[doc = concat!("//   10  MAX    (diff = 10 × 2^", stringify!($BITS), " + 2^", stringify!($BITS), " - 1)")]
        ///
        #[doc = concat!("let (a1, a0): (", stringify!($SelfT), ", ", stringify!($UnsignedT), ") = (6, 8);")]
        #[doc = concat!("let (b1, b0): (", stringify!($SelfT), ", ", stringify!($UnsignedT), ") = (-5, 9);")]
        /// let borrow0 = false;
        ///
        #[doc = concat!("// ", stringify!($UnsignedT), "::borrowing_sub for the less significant words")]
        /// let (diff0, borrow1) = a0.borrowing_sub(b0, borrow0);
        /// assert_eq!(borrow1, true);
        ///
        #[doc = concat!("// ", stringify!($SelfT), "::borrowing_sub for the most significant word")]
        /// let (diff1, overflow) = a1.borrowing_sub(b1, borrow1);
        /// assert_eq!(overflow, false);
        ///
        #[doc = concat!("assert_eq!((diff1, diff0), (10, ", stringify!($UnsignedT), "::MAX));")]
        /// ```
        #[unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn borrowing_sub(self, rhs: Self, borrow: bool) -> (Self, bool) {
            // 注意：长期来看这里应该通过 intrinsic 实现。
            // 注意：不需要中间溢出（https://github.com/rust-lang/rust/issues/85532#issuecomment-1032214946）。
            let (a, b) = self.overflowing_sub(rhs);
            let (c, d) = a.overflowing_sub(borrow as $SelfT);
            (c, b != d)
        }

        /// 计算 `self` - `rhs`，其中 `rhs` 是无符号整数。
        ///
        /// 返回减法结果以及一个表示是否会发生算术溢出的布尔值。
        /// 如果会发生溢出，则返回回绕后的值。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".overflowing_sub_unsigned(2), (-1, false));")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX).overflowing_sub_unsigned(", stringify!($UnsignedT), "::MAX), (", stringify!($SelfT), "::MIN, false));")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN + 2).overflowing_sub_unsigned(3), (", stringify!($SelfT), "::MAX, true));")]
        /// ```
        #[stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn overflowing_sub_unsigned(self, rhs: $UnsignedT) -> (Self, bool) {
            let rhs = rhs as Self;
            let (res, overflowed) = self.overflowing_sub(rhs);
            (res, overflowed ^ (rhs < 0))
        }

        /// 计算 `self` 和 `rhs` 的乘法。
        ///
        /// 返回乘法结果以及一个表示是否会发生算术溢出的布尔值。
        /// 如果会发生溢出，则返回回绕后的值。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".overflowing_mul(2), (10, false));")]
        /// assert_eq!(1_000_000_000i32.overflowing_mul(10), (1410065408, true));
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn overflowing_mul(self, rhs: Self) -> (Self, bool) {
            let (a, b) = intrinsics::mul_with_overflow(self as $ActualT, rhs as $ActualT);
            (a as Self, b)
        }

        /// 计算完整的乘积 `self * rhs`，不会发生溢出。
        ///
        /// 这会把结果的低位（回绕）部分和高位（溢出）部分作为两个独立值返回，
        /// 顺序也如此。
        ///
        /// 如果还需要向宽结果中加入进位，请改用 [`Self::carrying_mul`]。
        ///
        /// # 示例
        ///
        /// 请注意，此示例在各整数类型之间共享，因此这里使用 `i32`。
        ///
        /// ```
        /// #![feature(bigint_helper_methods)]
        /// assert_eq!(5i32.widening_mul(-2), (4294967286, -1));
        /// assert_eq!(1_000_000_000i32.widening_mul(-10), (2884901888, -3));
        /// ```
        #[unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[rustc_const_unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn widening_mul(self, rhs: Self) -> ($UnsignedT, Self) {
            Self::carrying_mul_add(self, rhs, 0, 0)
        }

        /// 计算“完整乘法”`self * rhs + carry`，不会发生溢出。
        ///
        /// 这会把结果的低位（回绕）部分和高位（溢出）部分作为两个独立值返回，
        /// 顺序也如此。
        ///
        /// 执行“长乘法”，它接收一个额外的待加数，并可能返回额外的溢出量。
        /// 这允许把多次乘法串接起来，创建表示更大数值的“大整数”。
        ///
        /// 如果不需要 `carry`，则可以改用 [`Self::widening_mul`]。
        ///
        /// # 示例
        ///
        /// 请注意，此示例在各整数类型之间共享，因此这里使用 `i32`。
        ///
        /// ```
        /// #![feature(bigint_helper_methods)]
        /// assert_eq!(5i32.carrying_mul(-2, 0), (4294967286, -1));
        /// assert_eq!(5i32.carrying_mul(-2, 10), (0, 0));
        /// assert_eq!(1_000_000_000i32.carrying_mul(-10, 0), (2884901888, -3));
        /// assert_eq!(1_000_000_000i32.carrying_mul(-10, 10), (2884901898, -3));
        #[doc = concat!("assert_eq!(",
            stringify!($SelfT), "::MAX.carrying_mul(", stringify!($SelfT), "::MAX, ", stringify!($SelfT), "::MAX), ",
            "(", stringify!($SelfT), "::MAX.unsigned_abs() + 1, ", stringify!($SelfT), "::MAX / 2));"
        )]
        /// ```
        #[unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[rustc_const_unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn carrying_mul(self, rhs: Self, carry: Self) -> ($UnsignedT, Self) {
            Self::carrying_mul_add(self, rhs, carry, 0)
        }

        /// 计算“完整乘法”`self * rhs + carry + add`，不会发生溢出。
        ///
        /// 这会把结果的低位（回绕）部分和高位（溢出）部分作为两个独立值返回，
        /// 顺序也如此。
        ///
        /// 执行“长乘法”，它接收一个额外的待加数，并可能返回额外的溢出量。
        /// 这允许把多次乘法串接起来，创建表示更大数值的“大整数”。
        ///
        /// 如果两个 `carry` 都不需要，则可以改用 [`Self::widening_mul`]；
        /// 如果只需要一个 `carry`，则可以改用 [`Self::carrying_mul`]。
        ///
        /// # 示例
        ///
        /// 请注意，此示例在各整数类型之间共享，因此这里使用 `i32`。
        ///
        /// ```
        /// #![feature(bigint_helper_methods)]
        /// assert_eq!(5i32.carrying_mul_add(-2, 0, 0), (4294967286, -1));
        /// assert_eq!(5i32.carrying_mul_add(-2, 10, 10), (10, 0));
        /// assert_eq!(1_000_000_000i32.carrying_mul_add(-10, 0, 0), (2884901888, -3));
        /// assert_eq!(1_000_000_000i32.carrying_mul_add(-10, 10, 10), (2884901908, -3));
        #[doc = concat!("assert_eq!(",
            stringify!($SelfT), "::MAX.carrying_mul_add(", stringify!($SelfT), "::MAX, ", stringify!($SelfT), "::MAX, ", stringify!($SelfT), "::MAX), ",
            "(", stringify!($UnsignedT), "::MAX, ", stringify!($SelfT), "::MAX / 2));"
        )]
        /// ```
        #[unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[rustc_const_unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn carrying_mul_add(self, rhs: Self, carry: Self, add: Self) -> ($UnsignedT, Self) {
            intrinsics::carrying_mul_add(self, rhs, carry, add)
        }

        /// 计算 `self` 除以 `rhs` 所得的商。
        ///
        /// 返回商以及一个表示是否会发生算术溢出的布尔值。
        /// 如果会发生溢出，则返回 self。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数将会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".overflowing_div(2), (2, false));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.overflowing_div(-1), (", stringify!($SelfT), "::MIN, true));")]
        /// ```
        #[inline]
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_overflowing_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        pub const fn overflowing_div(self, rhs: Self) -> (Self, bool) {
            // 使用 `&` 有助于 LLVM 识别这与除法中的检查相同。
            if intrinsics::unlikely((self == Self::MIN) & (rhs == -1)) {
                (self, true)
            } else {
                (self / rhs, false)
            }
        }

        /// 计算欧几里得除法 `self.div_euclid(rhs)` 的商。
        ///
        /// 返回商以及一个表示是否会发生算术溢出的布尔值。
        /// 如果会发生溢出，则返回 `self`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数将会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".overflowing_div_euclid(2), (2, false));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.overflowing_div_euclid(-1), (", stringify!($SelfT), "::MIN, true));")]
        /// ```
        #[inline]
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        pub const fn overflowing_div_euclid(self, rhs: Self) -> (Self, bool) {
            // 使用 `&` 有助于 LLVM 识别这与除法中的检查相同。
            if intrinsics::unlikely((self == Self::MIN) & (rhs == -1)) {
                (self, true)
            } else {
                (self.div_euclid(rhs), false)
            }
        }

        /// 计算 `self` 除以 `rhs` 时的余数。
        ///
        /// 返回除法后的余数以及一个表示是否会发生算术溢出的布尔值。
        /// 如果会发生溢出，则返回 0。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数将会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".overflowing_rem(2), (1, false));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.overflowing_rem(-1), (0, true));")]
        /// ```
        #[inline]
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_overflowing_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        pub const fn overflowing_rem(self, rhs: Self) -> (Self, bool) {
            if intrinsics::unlikely(rhs == -1) {
                (0, self == Self::MIN)
            } else {
                (self % rhs, false)
            }
        }


        /// 溢出型欧几里得取余。计算 `self.rem_euclid(rhs)`。
        ///
        /// 返回除法后的余数以及一个表示是否会发生算术溢出的布尔值。
        /// 如果会发生溢出，则返回 0。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数将会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".overflowing_rem_euclid(2), (1, false));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.overflowing_rem_euclid(-1), (0, true));")]
        /// ```
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn overflowing_rem_euclid(self, rhs: Self) -> (Self, bool) {
            if intrinsics::unlikely(rhs == -1) {
                (0, self == Self::MIN)
            } else {
                (self.rem_euclid(rhs), false)
            }
        }


        /// 对 `self` 取负；如果它等于最小值，则报告溢出。
        ///
        /// 返回取负后的值和一个布尔值；该布尔值表示是否发生溢出。
        /// 如果 `self` 是最小值（例如 `i32` 值的 `i32::MIN`），结果仍会返回最小值，
        /// 并用 `true` 表示发生了溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(2", stringify!($SelfT), ".overflowing_neg(), (-2, false));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.overflowing_neg(), (", stringify!($SelfT), "::MIN, true));")]
        /// ```
        #[inline]
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[allow(unused_attributes)]
        pub const fn overflowing_neg(self) -> (Self, bool) {
            if intrinsics::unlikely(self == Self::MIN) {
                (Self::MIN, true)
            } else {
                (-self, false)
            }
        }

        /// 将 `self` 左移 `rhs` 位。
        ///
        /// 返回移位后的值和一个布尔值；该布尔值表示移位量是否大于或等于位数。
        /// 如果移位量过大，则会用 `N - 1` 对它取掩码，其中 `N` 是该类型的位数，
        /// 然后用掩码后的值执行移位。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0x1", stringify!($SelfT),".overflowing_shl(4), (0x10, false));")]
        /// assert_eq!(0x1i32.overflowing_shl(36), (0x10, true));
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".overflowing_shl(", stringify!($BITS_MINUS_ONE), "), (0, false));")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn overflowing_shl(self, rhs: u32) -> (Self, bool) {
            (self.wrapping_shl(rhs), rhs >= Self::BITS)
        }

        /// 将 `self` 右移 `rhs` 位。
        ///
        /// 返回移位后的值和一个布尔值；该布尔值表示移位量是否大于或等于位数。
        /// 如果移位量过大，则会用 `N - 1` 对它取掩码，其中 `N` 是该类型的位数，
        /// 然后用掩码后的值执行移位。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".overflowing_shr(4), (0x1, false));")]
        /// assert_eq!(0x10i32.overflowing_shr(36), (0x1, true));
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn overflowing_shr(self, rhs: u32) -> (Self, bool) {
            (self.wrapping_shr(rhs), rhs >= Self::BITS)
        }

        /// 计算 `self` 的绝对值。
        ///
        /// 返回绝对值和一个布尔值；该布尔值表示是否发生溢出。如果 `self` 是最小值
        #[doc = concat!("(e.g., ", stringify!($SelfT), "::MIN for values of type ", stringify!($SelfT), "),")]
        /// 则仍返回最小值，并用 `true` 表示发生了溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(10", stringify!($SelfT), ".overflowing_abs(), (10, false));")]
        #[doc = concat!("assert_eq!((-10", stringify!($SelfT), ").overflowing_abs(), (10, false));")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MIN).overflowing_abs(), (", stringify!($SelfT), "::MIN, true));")]
        /// ```
        #[stable(feature = "no_panic_abs", since = "1.13.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn overflowing_abs(self) -> (Self, bool) {
            (self.wrapping_abs(), self == Self::MIN)
        }

        /// 使用平方求幂计算 `self` 的 `exp` 次方。
        ///
        /// 返回幂运算结果和一个布尔值；该布尔值表示计算过程中是否发生过溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(3", stringify!($SelfT), ".overflowing_pow(4), (81, false));")]
        #[doc = concat!("assert_eq!(0_", stringify!($SelfT), ".overflowing_pow(0), (1, false));")]
        /// assert_eq!(3i8.overflowing_pow(5), (-13, true));
        /// ```
        #[stable(feature = "no_panic_pow", since = "1.34.0")]
        #[rustc_const_stable(feature = "const_int_pow", since = "1.50.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn overflowing_pow(self, mut exp: u32) -> (Self, bool) {
            if exp == 0 {
                return (1,false);
            }
            let mut base = self;
            let mut acc: Self = 1;
            let mut overflown = false;
            // 用于暂存 `overflowing_mul` 结果的临时空间。
            let mut r;

            loop {
                if (exp & 1) == 1 {
                    r = acc.overflowing_mul(base);
                    // 因为 `exp != 0`，最终 `exp` 必然会降到 1。
                    if exp == 1 {
                        r.1 |= overflown;
                        return r;
                    }
                    acc = r.0;
                    overflown |= r.1;
                }
                exp /= 2;
                r = base.overflowing_mul(base);
                base = r.0;
                overflown |= r.1;
            }
        }

        /// 使用平方求幂计算 `self` 的 `exp` 次方。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let x: ", stringify!($SelfT), " = 2; // 或任何其他整数类型")]
        ///
        /// assert_eq!(x.pow(5), 32);
        #[doc = concat!("assert_eq!(0_", stringify!($SelfT), ".pow(0), 1);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_pow", since = "1.50.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[rustc_inherit_overflow_checks]
        pub const fn pow(self, mut exp: u32) -> Self {
            if exp == 0 {
                return 1;
            }
            let mut base = self;
            let mut acc = 1;

            if intrinsics::is_val_statically_known(exp) {
                while exp > 1 {
                    if (exp & 1) == 1 {
                        acc = acc * base;
                    }
                    exp /= 2;
                    base = base * base;
                }

                // 因为 `exp != 0`，最终 `exp` 必然会降到 1。
                // 单独处理指数的最后一位，因为之后不再需要对底数平方，
                // 而继续平方可能造成不必要的溢出。
                acc * base
            } else {
                // 当指数不是编译期已知值时，这比上面的路径更快。常量指数情形不能复用这段代码，
                // 因为 LLVM 目前无法展开这个循环。
                loop {
                    if (exp & 1) == 1 {
                        acc = acc * base;
                        // 因为 `exp != 0`，最终 `exp` 必然会降到 1。
                        if exp == 1 {
                            return acc;
                        }
                    }
                    exp /= 2;
                    base = base * base;
                }
            }
        }

        /// 返回该数的平方根，并向下舍入。
        ///
        /// # Panics
        ///
        /// 如果 `self` 为负数，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(10", stringify!($SelfT), ".isqrt(), 3);")]
        /// ```
        #[stable(feature = "isqrt", since = "1.84.0")]
        #[rustc_const_stable(feature = "isqrt", since = "1.84.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn isqrt(self) -> Self {
            match self.checked_isqrt() {
                Some(sqrt) => sqrt,
                None => crate::num::int_sqrt::panic_for_negative_argument(),
            }
        }

        /// 计算 `self` 除以 `rhs` 的 Euclidean 除法商。
        ///
        /// 它会计算满足 `self = q * rhs + r` 的整数 `q`，其中
        /// `r = self.rem_euclid(rhs)` 且 `0 <= r < abs(rhs)`。
        ///
        /// 换句话说，结果是把 `self / rhs` 舍入到满足 `self >= q * rhs` 的整数 `q`。
        /// 如果 `self > 0`，这等同于向零舍入（Rust 默认行为）；
        /// 如果 `self < 0`，这等同于远离零舍入（朝 +/- infinity）。
        /// 如果 `rhs > 0`，这等同于向 -infinity 舍入；
        /// 如果 `rhs < 0`，这等同于向 +infinity 舍入。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，或 `self` 为 `Self::MIN` 且 `rhs` 为 -1，
        /// 此函数会 panic。该行为不受 `overflow-checks` 标志影响。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let a: ", stringify!($SelfT), " = 7; // 或任何其他整数类型")]
        /// let b = 4;
        ///
        /// assert_eq!(a.div_euclid(b), 1); // 7 >= 4 * 1
        /// assert_eq!(a.div_euclid(-b), -1); // 7 >= -4 * -1
        /// assert_eq!((-a).div_euclid(b), -2); // -7 >= 4 * -2
        /// assert_eq!((-a).div_euclid(-b), 2); // -7 >= -4 * 2
        /// ```
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn div_euclid(self, rhs: Self) -> Self {
            let q = self / rhs;
            if self % rhs < 0 {
                return if rhs > 0 { q - 1 } else { q + 1 }
            }
            q
        }


        /// 计算 `self` 除以 `rhs` 后的最小非负余数。
        ///
        /// 这就像使用 Euclidean 除法算法完成：给定
        /// `r = self.rem_euclid(rhs)`，结果满足
        /// `self = rhs * self.div_euclid(rhs) + r` 且 `0 <= r < abs(rhs)`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，或 `self` 为 `Self::MIN` 且 `rhs` 为 -1，
        /// 此函数会 panic。该行为不受 `overflow-checks` 标志影响。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let a: ", stringify!($SelfT), " = 7; // 或任何其他整数类型")]
        /// let b = 4;
        ///
        /// assert_eq!(a.rem_euclid(b), 3);
        /// assert_eq!((-a).rem_euclid(b), 1);
        /// assert_eq!(a.rem_euclid(-b), 3);
        /// assert_eq!((-a).rem_euclid(-b), 1);
        /// ```
        ///
        /// 以下代码会 panic：
        /// ```should_panic
        #[doc = concat!("let _ = ", stringify!($SelfT), "::MIN.rem_euclid(-1);")]
        /// ```
        #[doc(alias = "modulo", alias = "mod")]
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn rem_euclid(self, rhs: Self) -> Self {
            let r = self % rhs;
            if r < 0 {
                // 在语义上等价于 `if rhs < 0 { r - rhs } else { r + rhs }`。
                // 如果 `rhs` 不是 `Self::MIN`，那么 `r + abs(rhs)` 不会溢出；
                // 因为 `r` 为负数，这显然等价。
                // 否则 `rhs` 是 `Self::MIN`，此时有
                // `r.wrapping_add(Self::MIN.wrapping_abs())`，它会求值为
                // `r.wrapping_add(Self::MIN)`，等价于我们想要的 `r - Self::MIN`
                // （对于负的 `r` 不会溢出）。
                r.wrapping_add(rhs.wrapping_abs())
            } else {
                r
            }
        }

        /// 计算 `self` 与 `rhs` 的商，并将结果向负无穷舍入。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，或 `self` 为 `Self::MIN` 且 `rhs` 为 -1，
        /// 此函数会 panic。该行为不受 `overflow-checks` 标志影响。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(int_roundings)]
        #[doc = concat!("let a: ", stringify!($SelfT)," = 8;")]
        /// let b = 3;
        ///
        /// assert_eq!(a.div_floor(b), 2);
        /// assert_eq!(a.div_floor(-b), -3);
        /// assert_eq!((-a).div_floor(b), -3);
        /// assert_eq!((-a).div_floor(-b), 2);
        /// ```
        #[unstable(feature = "int_roundings", issue = "88581")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn div_floor(self, rhs: Self) -> Self {
            let d = self / rhs;
            let r = self % rhs;

            // 如果余数非零，且 self 与 rhs 的符号不同，就需要减一；
            // 这表示我们刚才向上而不是向下舍入了。这里通过构造一个无分支掩码完成校正：
            // 当符号不同时掩码全为 1，否则为 0。加上这个掩码
            // （对应有符号值 -1）即可得到校正量。
            let correction = (self ^ rhs) >> (Self::BITS - 1);
            if r != 0 {
                d + correction
            } else {
                d
            }
        }

        /// 计算 `self` 与 `rhs` 的商，并将结果向正无穷舍入。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，或 `self` 为 `Self::MIN` 且 `rhs` 为 -1，
        /// 此函数会 panic。该行为不受 `overflow-checks` 标志影响。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(int_roundings)]
        #[doc = concat!("let a: ", stringify!($SelfT)," = 8;")]
        /// let b = 3;
        ///
        /// assert_eq!(a.div_ceil(b), 3);
        /// assert_eq!(a.div_ceil(-b), -2);
        /// assert_eq!((-a).div_ceil(b), -2);
        /// assert_eq!((-a).div_ceil(-b), 3);
        /// ```
        #[unstable(feature = "int_roundings", issue = "88581")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn div_ceil(self, rhs: Self) -> Self {
            let d = self / rhs;
            let r = self % rhs;

            // 当余数非零时，有 a.div_ceil(b) == 1 + a.div_floor(b)，
            // 因此可以复用 div_floor 的算法，只需额外加 1。
            let correction = 1 + ((self ^ rhs) >> (Self::BITS - 1));
            if r != 0 {
                d + correction
            } else {
                d
            }
        }

        /// 如果 `rhs` 为正，计算大于或等于 `self` 的最小 `rhs` 倍数。
        /// 如果 `rhs` 为负，计算小于或等于 `self` 的最大 `rhs` 倍数。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// ## Overflow behavior
        ///
        /// 发生溢出时，如果启用了溢出检查（debug 模式默认启用），此函数会 panic；
        /// 如果禁用了溢出检查（release 模式默认禁用），结果会回绕。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(int_roundings)]
        #[doc = concat!("assert_eq!(16_", stringify!($SelfT), ".next_multiple_of(8), 16);")]
        #[doc = concat!("assert_eq!(23_", stringify!($SelfT), ".next_multiple_of(8), 24);")]
        #[doc = concat!("assert_eq!(16_", stringify!($SelfT), ".next_multiple_of(-8), 16);")]
        #[doc = concat!("assert_eq!(23_", stringify!($SelfT), ".next_multiple_of(-8), 16);")]
        #[doc = concat!("assert_eq!((-16_", stringify!($SelfT), ").next_multiple_of(8), -16);")]
        #[doc = concat!("assert_eq!((-23_", stringify!($SelfT), ").next_multiple_of(8), -16);")]
        #[doc = concat!("assert_eq!((-16_", stringify!($SelfT), ").next_multiple_of(-8), -16);")]
        #[doc = concat!("assert_eq!((-23_", stringify!($SelfT), ").next_multiple_of(-8), -24);")]
        /// ```
        #[unstable(feature = "int_roundings", issue = "88581")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[rustc_inherit_overflow_checks]
        pub const fn next_multiple_of(self, rhs: Self) -> Self {
            // 否则当 self == T::MIN 时，计算 `r` 会失败。
            if rhs == -1 {
                return self;
            }

            let r = self % rhs;
            let m = if (r > 0 && rhs < 0) || (r < 0 && rhs > 0) {
                r + rhs
            } else {
                r
            };

            if m == 0 {
                self
            } else {
                self + (rhs - m)
            }
        }

        /// 如果 `rhs` 为正，计算大于或等于 `self` 的最小 `rhs` 倍数。
        /// 如果 `rhs` 为负，计算小于或等于 `self` 的最大 `rhs` 倍数。
        /// 如果 `rhs` 为零，或该操作会导致溢出，则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(int_roundings)]
        #[doc = concat!("assert_eq!(16_", stringify!($SelfT), ".checked_next_multiple_of(8), Some(16));")]
        #[doc = concat!("assert_eq!(23_", stringify!($SelfT), ".checked_next_multiple_of(8), Some(24));")]
        #[doc = concat!("assert_eq!(16_", stringify!($SelfT), ".checked_next_multiple_of(-8), Some(16));")]
        #[doc = concat!("assert_eq!(23_", stringify!($SelfT), ".checked_next_multiple_of(-8), Some(16));")]
        #[doc = concat!("assert_eq!((-16_", stringify!($SelfT), ").checked_next_multiple_of(8), Some(-16));")]
        #[doc = concat!("assert_eq!((-23_", stringify!($SelfT), ").checked_next_multiple_of(8), Some(-16));")]
        #[doc = concat!("assert_eq!((-16_", stringify!($SelfT), ").checked_next_multiple_of(-8), Some(-16));")]
        #[doc = concat!("assert_eq!((-23_", stringify!($SelfT), ").checked_next_multiple_of(-8), Some(-24));")]
        #[doc = concat!("assert_eq!(1_", stringify!($SelfT), ".checked_next_multiple_of(0), None);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.checked_next_multiple_of(2), None);")]
        /// ```
        #[unstable(feature = "int_roundings", issue = "88581")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_next_multiple_of(self, rhs: Self) -> Option<Self> {
            // 否则当 self == T::MIN 时，计算 `r` 会失败。
            if rhs == -1 {
                return Some(self);
            }

            let r = try_opt!(self.checked_rem(rhs));
            let m = if (r > 0 && rhs < 0) || (r < 0 && rhs > 0) {
                // `r` 与 `rhs` 符号相反，因此 `r + rhs` 不会溢出。
                r + rhs
            } else {
                r
            };

            if m == 0 {
                Some(self)
            } else {
                // `m` 与 `rhs` 符号相同，因此 `rhs - m` 不会溢出。
                self.checked_add(rhs - m)
            }
        }

        /// 返回该数相对于任意进制底数的对数，并向下舍入。
        ///
        /// 受实现细节影响，此方法可能没有完全优化；底数为 2 时 [`ilog2`] 更高效，
        /// 底数为 10 时 [`ilog10`] 更高效。
        ///
        /// # Panics
        ///
        /// 如果 `self` 小于或等于零，或 `base` 小于 2，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".ilog(5), 1);")]
        /// ```
        #[stable(feature = "int_log", since = "1.67.0")]
        #[rustc_const_stable(feature = "int_log", since = "1.67.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn ilog(self, base: Self) -> u32 {
            assert!(base >= 2, "base of integer logarithm must be at least 2");
            if let Some(log) = self.checked_ilog(base) {
                log
            } else {
                int_log10::panic_for_nonpositive_argument()
            }
        }

        /// 返回该数以 2 为底的对数，并向下舍入。
        ///
        /// # Panics
        ///
        /// 如果 `self` 小于或等于零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(2", stringify!($SelfT), ".ilog2(), 1);")]
        /// ```
        #[stable(feature = "int_log", since = "1.67.0")]
        #[rustc_const_stable(feature = "int_log", since = "1.67.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn ilog2(self) -> u32 {
            if let Some(log) = self.checked_ilog2() {
                log
            } else {
                int_log10::panic_for_nonpositive_argument()
            }
        }

        /// 返回该数以 10 为底的对数，并向下舍入。
        ///
        /// # Panics
        ///
        /// 如果 `self` 小于或等于零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(10", stringify!($SelfT), ".ilog10(), 1);")]
        /// ```
        #[stable(feature = "int_log", since = "1.67.0")]
        #[rustc_const_stable(feature = "int_log", since = "1.67.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn ilog10(self) -> u32 {
            if let Some(log) = self.checked_ilog10() {
                log
            } else {
                int_log10::panic_for_nonpositive_argument()
            }
        }

        /// 返回该数相对于任意进制底数的对数，并向下舍入。
        ///
        /// 如果该数为负数或零，或底数小于 2，则返回 `None`。
        ///
        /// 受实现细节影响，此方法可能没有完全优化；底数为 2 时 [`checked_ilog2`] 更高效，
        /// 底数为 10 时 [`checked_ilog10`] 更高效。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".checked_ilog(5), Some(1));")]
        /// ```
        #[stable(feature = "int_log", since = "1.67.0")]
        #[rustc_const_stable(feature = "int_log", since = "1.67.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_ilog(self, base: Self) -> Option<u32> {
            if self <= 0 || base <= 1 {
                None
            } else {
                // 委托给无符号实现。
                // 前面的条件确保两次转换都是精确的。
                (self as $UnsignedT).checked_ilog(base as $UnsignedT)
            }
        }

        /// 返回该数以 2 为底的对数，并向下舍入。
        ///
        /// 如果该数为负数或零，则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(2", stringify!($SelfT), ".checked_ilog2(), Some(1));")]
        /// ```
        #[stable(feature = "int_log", since = "1.67.0")]
        #[rustc_const_stable(feature = "int_log", since = "1.67.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_ilog2(self) -> Option<u32> {
            if self <= 0 {
                None
            } else {
                // SAFETY: 刚刚已经检查过该数为正数，因此满足 `ctlz_nonzero` 的非零前置条件。
                let log = (Self::BITS - 1) - unsafe { intrinsics::ctlz_nonzero(self) as u32 };
                Some(log)
            }
        }

        /// 返回该数以 10 为底的对数，并向下舍入。
        ///
        /// 如果该数为负数或零，则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(10", stringify!($SelfT), ".checked_ilog10(), Some(1));")]
        /// ```
        #[stable(feature = "int_log", since = "1.67.0")]
        #[rustc_const_stable(feature = "int_log", since = "1.67.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_ilog10(self) -> Option<u32> {
            int_log10::$ActualT(self as $ActualT)
        }

        /// 计算 `self` 的绝对值。
        ///
        /// # 溢出行为
        ///
        /// 以下值的绝对值
        #[doc = concat!("`", stringify!($SelfT), "::MIN`")]
        /// 无法表示为
        #[doc = concat!("`", stringify!($SelfT), "`，")]
        /// 因而尝试计算它会导致溢出。这意味着 debug 模式下的代码遇到这种情况会触发 panic，
        /// 优化后的代码则会返回
        #[doc = concat!("`", stringify!($SelfT), "::MIN`")]
        /// 而不 panic。如果不想要这种行为，请考虑改用 [`unsigned_abs`](Self::unsigned_abs)。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(10", stringify!($SelfT), ".abs(), 10);")]
        #[doc = concat!("assert_eq!((-10", stringify!($SelfT), ").abs(), 10);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[allow(unused_attributes)]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[rustc_inherit_overflow_checks]
        pub const fn abs(self) -> Self {
            // 注意，上面的 #[rustc_inherit_overflow_checks] 和 #[inline]
            // 意味着这个取负操作的溢出语义取决于调用它的 crate。
            if self.is_negative() {
                -self
            } else {
                self
            }
        }

        /// 计算 `self` 与 `other` 之间的绝对差值。
        ///
        /// 该函数返回无符号整数，因此总能在不溢出、不 panic 的情况下返回正确结果。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".abs_diff(80), 20", stringify!($UnsignedT), ");")]
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".abs_diff(110), 10", stringify!($UnsignedT), ");")]
        #[doc = concat!("assert_eq!((-100", stringify!($SelfT), ").abs_diff(80), 180", stringify!($UnsignedT), ");")]
        #[doc = concat!("assert_eq!((-100", stringify!($SelfT), ").abs_diff(-120), 20", stringify!($UnsignedT), ");")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN.abs_diff(", stringify!($SelfT), "::MAX), ", stringify!($UnsignedT), "::MAX);")]
        /// ```
        #[stable(feature = "int_abs_diff", since = "1.60.0")]
        #[rustc_const_stable(feature = "int_abs_diff", since = "1.60.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn abs_diff(self, other: Self) -> $UnsignedT {
            if self < other {
                // 使用 `x as U` 将非负 x 从有符号转为无符号时，数值保持不变；
                // 负的 x 则会转换为 x + 2^N。因此，如果 `s` 和 `o` 是二值变量，
                // 分别表示 `self` 和 `other` 是否为负数，我们计算的是以下数学值：
                //
                //    (other + o*2^N) - (self + s*2^N)    mod  2^N
                //    other - self + (o-s)*2^N            mod  2^N
                //    other - self                        mod  2^N
                //
                // 最后，对 `other - self` 的数学值取 mod 2^N 不会改变它，
                // 因为它已经位于 [0, 2^N) 范围内。
                (other as $UnsignedT).wrapping_sub(self as $UnsignedT)
            } else {
                (self as $UnsignedT).wrapping_sub(other as $UnsignedT)
            }
        }

        /// 返回一个表示 `self` 符号的数字。
        ///
        ///  - 如果该数为零，返回 `0`
        ///  - 如果该数为正，返回 `1`
        ///  - 如果该数为负，返回 `-1`
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(10", stringify!($SelfT), ".signum(), 1);")]
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".signum(), 0);")]
        #[doc = concat!("assert_eq!((-10", stringify!($SelfT), ").signum(), -1);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_sign", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn signum(self) -> Self {
            // 为这个操作选择合适写法很复杂
            // (<https://graphics.stanford.edu/~seander/bithacks.html#CopyIntegerSign>)
            // 因此委托给 `Ord`。它已经能精确产出我们需要的 -1/0/+1，
            // 也适合作为处理这些复杂性的地方。

            crate::intrinsics::three_way_compare(self, 0) as Self
        }

        /// 如果 `self` 为正数，则返回 `true`；如果该数为零或负数，则返回 `false`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert!(10", stringify!($SelfT), ".is_positive());")]
        #[doc = concat!("assert!(!(-10", stringify!($SelfT), ").is_positive());")]
        /// ```
        #[must_use]
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[inline(always)]
        pub const fn is_positive(self) -> bool { self > 0 }

        /// 如果 `self` 为负数，则返回 `true`；如果该数为零或正数，则返回 `false`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert!((-10", stringify!($SelfT), ").is_negative());")]
        #[doc = concat!("assert!(!10", stringify!($SelfT), ".is_negative());")]
        /// ```
        #[must_use]
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_methods", since = "1.32.0")]
        #[inline(always)]
        pub const fn is_negative(self) -> bool { self < 0 }

        /// 以 big-endian（网络）字节序字节数组的形式返回此整数的内存表示。
        ///
        #[doc = $to_xe_bytes_doc]
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let bytes = ", $swap_op, stringify!($SelfT), ".to_be_bytes();")]
        #[doc = concat!("assert_eq!(bytes, ", $be_bytes, ");")]
        /// ```
        #[stable(feature = "int_to_from_bytes", since = "1.32.0")]
        #[rustc_const_stable(feature = "const_int_conversion", since = "1.44.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn to_be_bytes(self) -> [u8; size_of::<Self>()] {
            self.to_be().to_ne_bytes()
        }

        /// 以 little-endian 字节序字节数组的形式返回此整数的内存表示。
        ///
        #[doc = $to_xe_bytes_doc]
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let bytes = ", $swap_op, stringify!($SelfT), ".to_le_bytes();")]
        #[doc = concat!("assert_eq!(bytes, ", $le_bytes, ");")]
        /// ```
        #[stable(feature = "int_to_from_bytes", since = "1.32.0")]
        #[rustc_const_stable(feature = "const_int_conversion", since = "1.44.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn to_le_bytes(self) -> [u8; size_of::<Self>()] {
            self.to_le().to_ne_bytes()
        }

        /// 以原生字节序字节数组的形式返回此整数的内存表示。
        ///
        /// 由于这里使用目标平台的原生字节序，可移植代码通常应酌情改用
        /// [`to_be_bytes`] 或 [`to_le_bytes`]。
        ///
        #[doc = $to_xe_bytes_doc]
        ///
        /// [`to_be_bytes`]: Self::to_be_bytes
        /// [`to_le_bytes`]: Self::to_le_bytes
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let bytes = ", $swap_op, stringify!($SelfT), ".to_ne_bytes();")]
        /// assert_eq!(
        ///     bytes,
        ///     if cfg!(target_endian = "big") {
        #[doc = concat!("        ", $be_bytes)]
        ///     } else {
        #[doc = concat!("        ", $le_bytes)]
        ///     }
        /// );
        /// ```
        #[stable(feature = "int_to_from_bytes", since = "1.32.0")]
        #[rustc_const_stable(feature = "const_int_conversion", since = "1.44.0")]
        #[allow(unnecessary_transmutes)]
        // SAFETY: 这个 const 转换是健全的，因为整数是 plain old datatype，
        // 所有位模式都有效，因此总能 transmute 为字节数组。
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn to_ne_bytes(self) -> [u8; size_of::<Self>()] {
            // SAFETY: 整数是 plain old datatype，所有位模式都有效，因此总能 transmute 为字节数组。
            unsafe { mem::transmute(self) }
        }

        /// 从 big endian 字节数组表示创建一个整数值。
        ///
        #[doc = $from_xe_bytes_doc]
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let value = ", stringify!($SelfT), "::from_be_bytes(", $be_bytes, ");")]
        #[doc = concat!("assert_eq!(value, ", $swap_op, ");")]
        /// ```
        ///
        /// 如果起点是切片而不是数组，可以使用可失败转换 API：
        ///
        /// ```
        #[doc = concat!("fn read_be_", stringify!($SelfT), "(input: &mut &[u8]) -> ", stringify!($SelfT), " {")]
        #[doc = concat!("    let (int_bytes, rest) = input.split_at(size_of::<", stringify!($SelfT), ">());")]
        ///     *input = rest;
        #[doc = concat!("    ", stringify!($SelfT), "::from_be_bytes(int_bytes.try_into().unwrap())")]
        /// }
        /// ```
        #[stable(feature = "int_to_from_bytes", since = "1.32.0")]
        #[rustc_const_stable(feature = "const_int_conversion", since = "1.44.0")]
        #[must_use]
        #[inline]
        pub const fn from_be_bytes(bytes: [u8; size_of::<Self>()]) -> Self {
            Self::from_be(Self::from_ne_bytes(bytes))
        }

        /// 从 little endian 字节数组表示创建一个整数值。
        ///
        #[doc = $from_xe_bytes_doc]
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let value = ", stringify!($SelfT), "::from_le_bytes(", $le_bytes, ");")]
        #[doc = concat!("assert_eq!(value, ", $swap_op, ");")]
        /// ```
        ///
        /// 如果起点是切片而不是数组，可以使用可失败转换 API：
        ///
        /// ```
        #[doc = concat!("fn read_le_", stringify!($SelfT), "(input: &mut &[u8]) -> ", stringify!($SelfT), " {")]
        #[doc = concat!("    let (int_bytes, rest) = input.split_at(size_of::<", stringify!($SelfT), ">());")]
        ///     *input = rest;
        #[doc = concat!("    ", stringify!($SelfT), "::from_le_bytes(int_bytes.try_into().unwrap())")]
        /// }
        /// ```
        #[stable(feature = "int_to_from_bytes", since = "1.32.0")]
        #[rustc_const_stable(feature = "const_int_conversion", since = "1.44.0")]
        #[must_use]
        #[inline]
        pub const fn from_le_bytes(bytes: [u8; size_of::<Self>()]) -> Self {
            Self::from_le(Self::from_ne_bytes(bytes))
        }

        /// 从原生字节序字节数组形式的内存表示创建一个整数值。
        ///
        /// 由于这里使用目标平台的原生字节序，可移植代码通常应酌情改用
        /// [`from_be_bytes`] 或 [`from_le_bytes`]。
        ///
        /// [`from_be_bytes`]: Self::from_be_bytes
        /// [`from_le_bytes`]: Self::from_le_bytes
        ///
        #[doc = $from_xe_bytes_doc]
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let value = ", stringify!($SelfT), "::from_ne_bytes(if cfg!(target_endian = \"big\") {")]
        #[doc = concat!("    ", $be_bytes)]
        /// } else {
        #[doc = concat!("    ", $le_bytes)]
        /// });
        #[doc = concat!("assert_eq!(value, ", $swap_op, ");")]
        /// ```
        ///
        /// 如果起点是切片而不是数组，可以使用可失败转换 API：
        ///
        /// ```
        #[doc = concat!("fn read_ne_", stringify!($SelfT), "(input: &mut &[u8]) -> ", stringify!($SelfT), " {")]
        #[doc = concat!("    let (int_bytes, rest) = input.split_at(size_of::<", stringify!($SelfT), ">());")]
        ///     *input = rest;
        #[doc = concat!("    ", stringify!($SelfT), "::from_ne_bytes(int_bytes.try_into().unwrap())")]
        /// }
        /// ```
        #[stable(feature = "int_to_from_bytes", since = "1.32.0")]
        #[rustc_const_stable(feature = "const_int_conversion", since = "1.44.0")]
        #[allow(unnecessary_transmutes)]
        #[must_use]
        // SAFETY: 这个 const 转换是健全的，因为整数是 plain old datatype，
        // 所有位模式都有效，因此总能从字节数组 transmute 为整数。
        #[inline]
        pub const fn from_ne_bytes(bytes: [u8; size_of::<Self>()]) -> Self {
            // SAFETY: 整数是 plain old datatype，所有位模式都有效，因此总能从字节数组 transmute 为整数。
            unsafe { mem::transmute(bytes) }
        }

        /// 新代码应优先使用
        #[doc = concat!("[`", stringify!($SelfT), "::MIN", "`]。")]
        ///
        /// 返回此整数类型可以表示的最小值。
        #[stable(feature = "rust1", since = "1.0.0")]
        #[inline(always)]
        #[rustc_promotable]
        #[rustc_const_stable(feature = "const_min_value", since = "1.32.0")]
        #[deprecated(since = "TBD", note = "replaced by the `MIN` associated constant on this type")]
        #[rustc_diagnostic_item = concat!(stringify!($SelfT), "_legacy_fn_min_value")]
        pub const fn min_value() -> Self {
            Self::MIN
        }

        /// 新代码应优先使用
        #[doc = concat!("[`", stringify!($SelfT), "::MAX", "`]。")]
        ///
        /// 返回此整数类型可以表示的最大值。
        #[stable(feature = "rust1", since = "1.0.0")]
        #[inline(always)]
        #[rustc_promotable]
        #[rustc_const_stable(feature = "const_max_value", since = "1.32.0")]
        #[deprecated(since = "TBD", note = "replaced by the `MAX` associated constant on this type")]
        #[rustc_diagnostic_item = concat!(stringify!($SelfT), "_legacy_fn_max_value")]
        pub const fn max_value() -> Self {
            Self::MAX
        }

        /// 将此数限制在以零为中心的对称范围内。
        ///
        /// 该方法会把数值的幅度（绝对值）限制为至多 `limit`。
        ///
        /// 从功能上看，这等价于 `self.clamp(-limit, limit)`，但意图表达得更明确。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(clamp_magnitude)]
        #[doc = concat!("assert_eq!(120", stringify!($SelfT), ".clamp_magnitude(100), 100);")]
        #[doc = concat!("assert_eq!(-120", stringify!($SelfT), ".clamp_magnitude(100), -100);")]
        #[doc = concat!("assert_eq!(80", stringify!($SelfT), ".clamp_magnitude(100), 80);")]
        #[doc = concat!("assert_eq!(-80", stringify!($SelfT), ".clamp_magnitude(100), -80);")]
        /// ```
        #[must_use = "this returns the clamped value and does not modify the original"]
        #[unstable(feature = "clamp_magnitude", issue = "148519")]
        #[inline]
        pub fn clamp_magnitude(self, limit: $UnsignedT) -> Self {
            if let Ok(limit) = core::convert::TryInto::<$SelfT>::try_into(limit) {
                self.clamp(-limit, limit)
            } else {
                self
            }
        }
    }
}
