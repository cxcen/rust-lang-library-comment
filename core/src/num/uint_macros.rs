macro_rules! uint_impl {
    (
        Self = $SelfT:ty,
        ActualT = $ActualT:ident,
        SignedT = $SignedT:ident,

        // 这些都只用于文档注释。
        // 因此它们都以字面量传入；如果需要表示多个代码 token，
        // 传入字符串字面量也是可以的。
        // 在非注释中应使用关联常量，而不是这些参数。
        BITS = $BITS:literal,
        BITS_MINUS_ONE = $BITS_MINUS_ONE:literal,
        MAX = $MaxV:literal,
        rot = $rot:literal,
        rot_op = $rot_op:literal,
        rot_result = $rot_result:literal,
        fsh_op = $fsh_op:literal,
        fshl_result = $fshl_result:literal,
        fshr_result = $fshr_result:literal,
        swap_op = $swap_op:literal,
        swapped = $swapped:literal,
        reversed = $reversed:literal,
        le_bytes = $le_bytes:literal,
        be_bytes = $be_bytes:literal,
        to_xe_bytes_doc = $to_xe_bytes_doc:expr,
        from_xe_bytes_doc = $from_xe_bytes_doc:expr,
        bound_condition = $bound_condition:literal,
    ) => {
        /// 此整数类型可以表示的最小值。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MIN, 0);")]
        /// ```
        #[stable(feature = "assoc_int_consts", since = "1.43.0")]
        pub const MIN: Self = 0;

        /// 此整数类型可以表示的最大值
        #[doc = concat!("(2<sup>", $BITS, "</sup> &minus; 1", $bound_condition, ").")]
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX, ", stringify!($MaxV), ");")]
        /// ```
        #[stable(feature = "assoc_int_consts", since = "1.43.0")]
        pub const MAX: Self = !0;

        /// 此整数类型的位数。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::BITS, ", stringify!($BITS), ");")]
        /// ```
        #[stable(feature = "int_bits_const", since = "1.53.0")]
        pub const BITS: u32 = Self::MAX.count_ones();

        /// 返回 `self` 二进制表示中的 1 的个数。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = 0b01001100", stringify!($SelfT), ";")]
        /// assert_eq!(n.count_ones(), 3);
        ///
        #[doc = concat!("let max = ", stringify!($SelfT),"::MAX;")]
        #[doc = concat!("assert_eq!(max.count_ones(), ", stringify!($BITS), ");")]
        ///
        #[doc = concat!("let zero = 0", stringify!($SelfT), ";")]
        /// assert_eq!(zero.count_ones(), 0);
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_math", since = "1.32.0")]
        #[doc(alias = "popcount")]
        #[doc(alias = "popcnt")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn count_ones(self) -> u32 {
            return intrinsics::ctpop(self);
        }

        /// 返回 `self` 二进制表示中的 0 的个数。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let zero = 0", stringify!($SelfT), ";")]
        #[doc = concat!("assert_eq!(zero.count_zeros(), ", stringify!($BITS), ");")]
        ///
        #[doc = concat!("let max = ", stringify!($SelfT),"::MAX;")]
        /// assert_eq!(max.count_zeros(), 0);
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn count_zeros(self) -> u32 {
            (!self).count_ones()
        }

        /// 返回 `self` 二进制表示中的前导 0 的个数。
        ///
        /// 取决于你要如何使用该值，你可能还会关注 [`ilog2`] 函数；
        /// 即使类型变宽，它也会返回一致的数值。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = ", stringify!($SelfT), "::MAX >> 2;")]
        /// assert_eq!(n.leading_zeros(), 2);
        ///
        #[doc = concat!("let zero = 0", stringify!($SelfT), ";")]
        #[doc = concat!("assert_eq!(zero.leading_zeros(), ", stringify!($BITS), ");")]
        ///
        #[doc = concat!("let max = ", stringify!($SelfT),"::MAX;")]
        /// assert_eq!(max.leading_zeros(), 0);
        /// ```
        #[doc = concat!("[`ilog2`]: ", stringify!($SelfT), "::ilog2")]
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn leading_zeros(self) -> u32 {
            return intrinsics::ctlz(self as $ActualT);
        }

        /// 返回 `self` 二进制表示中的尾随 0 的个数。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = 0b0101000", stringify!($SelfT), ";")]
        /// assert_eq!(n.trailing_zeros(), 3);
        ///
        #[doc = concat!("let zero = 0", stringify!($SelfT), ";")]
        #[doc = concat!("assert_eq!(zero.trailing_zeros(), ", stringify!($BITS), ");")]
        ///
        #[doc = concat!("let max = ", stringify!($SelfT),"::MAX;")]
        #[doc = concat!("assert_eq!(max.trailing_zeros(), 0);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn trailing_zeros(self) -> u32 {
            return intrinsics::cttz(self);
        }

        /// 返回 `self` 二进制表示中的前导 1 的个数。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = !(", stringify!($SelfT), "::MAX >> 2);")]
        /// assert_eq!(n.leading_ones(), 2);
        ///
        #[doc = concat!("let zero = 0", stringify!($SelfT), ";")]
        /// assert_eq!(zero.leading_ones(), 0);
        ///
        #[doc = concat!("let max = ", stringify!($SelfT),"::MAX;")]
        #[doc = concat!("assert_eq!(max.leading_ones(), ", stringify!($BITS), ");")]
        /// ```
        #[stable(feature = "leading_trailing_ones", since = "1.46.0")]
        #[rustc_const_stable(feature = "leading_trailing_ones", since = "1.46.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn leading_ones(self) -> u32 {
            (!self).leading_zeros()
        }

        /// 返回 `self` 二进制表示中的尾随 1 的个数。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = 0b1010111", stringify!($SelfT), ";")]
        /// assert_eq!(n.trailing_ones(), 3);
        ///
        #[doc = concat!("let zero = 0", stringify!($SelfT), ";")]
        /// assert_eq!(zero.trailing_ones(), 0);
        ///
        #[doc = concat!("let max = ", stringify!($SelfT),"::MAX;")]
        #[doc = concat!("assert_eq!(max.trailing_ones(), ", stringify!($BITS), ");")]
        /// ```
        #[stable(feature = "leading_trailing_ones", since = "1.46.0")]
        #[rustc_const_stable(feature = "leading_trailing_ones", since = "1.46.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn trailing_ones(self) -> u32 {
            (!self).trailing_zeros()
        }

        /// 返回表示 `self` 所需的最少位数。
        ///
        /// 如果 `self` 为零，此方法返回零。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(uint_bit_width)]
        ///
        #[doc = concat!("assert_eq!(0_", stringify!($SelfT), ".bit_width(), 0);")]
        #[doc = concat!("assert_eq!(0b111_", stringify!($SelfT), ".bit_width(), 3);")]
        #[doc = concat!("assert_eq!(0b1110_", stringify!($SelfT), ".bit_width(), 4);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.bit_width(), ", stringify!($BITS), ");")]
        /// ```
        #[unstable(feature = "uint_bit_width", issue = "142326")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn bit_width(self) -> u32 {
            Self::BITS - self.leading_zeros()
        }

        /// 返回只保留最高有效置位位的 `self`；如果输入为 `0`，则返回 `0`。
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

        /// 返回只保留最低有效置位位的 `self`；如果输入为 `0`，则返回 `0`。
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

        /// 返回 `self` 中最高置位位的索引；如果 `self` 为 `0`，则返回 `None`。
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
            match NonZero::new(self) {
                Some(v) => Some(v.highest_one()),
                None => None,
            }
        }

        /// 返回 `self` 中最低置位位的索引；如果 `self` 为 `0`，则返回 `None`。
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
            match NonZero::new(self) {
                Some(v) => Some(v.lowest_one()),
                None => None,
            }
        }

        /// 返回把 `self` 的位模式重新解释为同宽有符号整数后的值。
        ///
        /// 这会产生与 `as` 转换相同的结果，但保证位宽保持不变。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = ", stringify!($SelfT), "::MAX;")]
        ///
        #[doc = concat!("assert_eq!(n.cast_signed(), -1", stringify!($SignedT), ");")]
        /// ```
        #[stable(feature = "integer_sign_cast", since = "1.87.0")]
        #[rustc_const_stable(feature = "integer_sign_cast", since = "1.87.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn cast_signed(self) -> $SignedT {
            self as $SignedT
        }

        /// 将各个位向左旋转指定的位数 `n`，
        /// 被截断的位会回绕到结果整数的末端。
        ///
        /// `rotate_left(n)` 等价于总共应用 `n` 次 `rotate_left(1)`。
        /// 特别是，旋转 `self` 的位数这么多位会原样返回输入值。
        ///
        /// 请注意，这与 `<<` 移位运算符不是同一种操作！
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
        #[rustc_const_stable(feature = "const_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[rustc_allow_const_fn_unstable(const_trait_impl)] // for the intrinsic fallback
        pub const fn rotate_left(self, n: u32) -> Self {
            return intrinsics::rotate_left(self, n);
        }

        /// 将各个位向右旋转指定的位数 `n`，
        /// 被截断的位会回绕到结果整数的开头。
        ///
        /// `rotate_right(n)` 等价于总共应用 `n` 次 `rotate_right(1)`。
        /// 特别是，旋转 `self` 的位数这么多位会原样返回输入值。
        ///
        /// 请注意，这与 `>>` 移位运算符不是同一种操作！
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
        #[rustc_const_stable(feature = "const_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[rustc_allow_const_fn_unstable(const_trait_impl)] // for the intrinsic fallback
        pub const fn rotate_right(self, n: u32) -> Self {
            return intrinsics::rotate_right(self, n);
        }

        /// 执行左漏斗移位：将 `self` 与 `rhs` 拼接，其中 `self` 构成最高有效半段，
        /// 然后把组合值向左移动 `n` 位，并取最高有效半段作为结果。
        ///
        /// 请注意，这与 `<<` 移位运算符或 [`rotate_left`](Self::rotate_left)
        /// 不是同一种操作，尽管 `a.funnel_shl(a, n)` *等价于* `a.rotate_left(n)`。
        ///
        /// # Panics
        ///
        /// 如果 `n` 大于或等于 `self` 的位数，此函数会 panic。
        ///
        /// # 示例
        ///
        /// 基本用法：
        ///
        /// ```
        /// #![feature(funnel_shifts)]
        #[doc = concat!("let a = ", $rot_op, stringify!($SelfT), ";")]
        #[doc = concat!("let b = ", $fsh_op, stringify!($SelfT), ";")]
        #[doc = concat!("let m = ", $fshl_result, ";")]
        ///
        #[doc = concat!("assert_eq!(a.funnel_shl(b, ", $rot, "), m);")]
        /// ```
        #[rustc_const_unstable(feature = "funnel_shifts", issue = "145686")]
        #[unstable(feature = "funnel_shifts", issue = "145686")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn funnel_shl(self, rhs: Self, n: u32) -> Self {
            assert!(n < Self::BITS, "attempt to funnel shift left with overflow");
            // SAFETY: 上面的断言已经检查 `n` 在范围内；若越界调用未检查漏斗移位会产生 UB。
            unsafe { intrinsics::unchecked_funnel_shl(self, rhs, n) }
        }

        /// 执行右漏斗移位：将 `self` 与 `rhs` 拼接，其中 `self` 构成最高有效半段，
        /// 然后把组合值向右移动 `n` 位，并取最低有效半段作为结果。
        ///
        /// 请注意，这与 `>>` 移位运算符或 [`rotate_right`](Self::rotate_right)
        /// 不是同一种操作，尽管 `a.funnel_shr(a, n)` *等价于* `a.rotate_right(n)`。
        ///
        /// # Panics
        ///
        /// 如果 `n` 大于或等于 `self` 的位数，此函数会 panic。
        ///
        /// # 示例
        ///
        /// 基本用法：
        ///
        /// ```
        /// #![feature(funnel_shifts)]
        #[doc = concat!("let a = ", $rot_op, stringify!($SelfT), ";")]
        #[doc = concat!("let b = ", $fsh_op, stringify!($SelfT), ";")]
        #[doc = concat!("let m = ", $fshr_result, ";")]
        ///
        #[doc = concat!("assert_eq!(a.funnel_shr(b, ", $rot, "), m);")]
        /// ```
        #[rustc_const_unstable(feature = "funnel_shifts", issue = "145686")]
        #[unstable(feature = "funnel_shifts", issue = "145686")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn funnel_shr(self, rhs: Self, n: u32) -> Self {
            assert!(n < Self::BITS, "attempt to funnel shift right with overflow");
            // SAFETY: 上面的断言已经检查 `n` 在范围内；若越界调用未检查漏斗移位会产生 UB。
            unsafe { intrinsics::unchecked_funnel_shr(self, rhs, n) }
        }

        /// 反转此整数的字节顺序。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("let n = ", $swap_op, stringify!($SelfT), ";")]
        /// let m = n.swap_bytes();
        ///
        #[doc = concat!("assert_eq!(m, ", $swapped, ");")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn swap_bytes(self) -> Self {
            intrinsics::bswap(self as $ActualT) as Self
        }

        /// 返回一个整数，其中 `mask` 指定的位位置会被连续打包到结果的最低有效位中。
        /// ```
        /// #![feature(uint_gather_scatter_bits)]
        #[doc = concat!("let n: ", stringify!($SelfT), " = 0b1011_1100;")]
        ///
        /// assert_eq!(n.gather_bits(0b0010_0100), 0b0000_0011);
        /// assert_eq!(n.gather_bits(0xF0), 0b0000_1011);
        /// ```
        #[unstable(feature = "uint_gather_scatter_bits", issue = "149069")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn gather_bits(self, mask: Self) -> Self {
            crate::num::int_bits::$ActualT::gather_impl(self as $ActualT, mask as $ActualT) as $SelfT
        }

        /// 返回一个整数，其中 `self` 的最低有效位会分散到 `mask` 指定的位位置。
        /// ```
        /// #![feature(uint_gather_scatter_bits)]
        #[doc = concat!("let n: ", stringify!($SelfT), " = 0b1010_1101;")]
        ///
        /// assert_eq!(n.scatter_bits(0b0101_0101), 0b0101_0001);
        /// assert_eq!(n.scatter_bits(0xF0), 0b1101_0000);
        /// ```
        #[unstable(feature = "uint_gather_scatter_bits", issue = "149069")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn scatter_bits(self, mask: Self) -> Self {
            crate::num::int_bits::$ActualT::scatter_impl(self as $ActualT, mask as $ActualT) as $SelfT
        }

        /// 反转此整数中的位顺序。最低有效位会成为最高有效位，
        ///                 次低有效位会成为次高有效位，依此类推。
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
            intrinsics::bitreverse(self as $ActualT) as Self
        }

        /// 将一个整数从大端字节序转换为目标平台的字节序。
        ///
        /// 在大端平台上这不执行操作；在小端平台上会交换字节。
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
        #[rustc_const_stable(feature = "const_math", since = "1.32.0")]
        #[must_use]
        #[inline(always)]
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

        /// 将一个整数从小端字节序转换为目标平台的字节序。
        ///
        /// 在小端平台上这不执行操作；在大端平台上会交换字节。
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
        #[rustc_const_stable(feature = "const_math", since = "1.32.0")]
        #[must_use]
        #[inline(always)]
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

        /// 将 `self` 从目标平台字节序转换为大端字节序。
        ///
        /// 在大端平台上这不执行操作；在小端平台上会交换字节。
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
        #[rustc_const_stable(feature = "const_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn to_be(self) -> Self { // 生存还是毁灭？
            #[cfg(target_endian = "big")]
            {
                self
            }
            #[cfg(not(target_endian = "big"))]
            {
                self.swap_bytes()
            }
        }

        /// 将 `self` 从目标平台字节序转换为小端字节序。
        ///
        /// 在小端平台上这不执行操作；在大端平台上会交换字节。
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
        #[rustc_const_stable(feature = "const_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
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
        #[doc = concat!(
            "assert_eq!((", stringify!($SelfT), "::MAX - 2).checked_add(1), ",
            "Some(", stringify!($SelfT), "::MAX - 1));"
        )]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).checked_add(3), None);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_checked_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_add(self, rhs: Self) -> Option<Self> {
            // 这里曾经使用 `overflowing_add`，但那会最终变成 `wrapping_add`，
            // 丢失一些优化机会。值得注意的是，这种写法有助于把
            // `.checked_add(1)` 优化为对 `MAX` 的检查和一个 `add nuw`。
            // 根据 <https://github.com/rust-lang/rust/pull/124114#issuecomment-2066173305>，
            // 如果后续有用，LLVM 很乐意重新形成 intrinsic。

            if intrinsics::unlikely(intrinsics::add_with_overflow(self, rhs).1) {
                None
            } else {
                // SAFETY: 刚刚已经检查过不会溢出；未检查加法的前置条件已满足。
                Some(unsafe { intrinsics::unchecked_add(self, rhs) })
            }
        }

        /// 严格型整数加法。计算 `self + rhs`，如果发生溢出则 panic。
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

        /// 未检查整数加法。计算 `self + rhs`，并假定不会发生溢出。
        ///
        /// 调用 `x.unchecked_add(y)` 在语义上等价于调用
        /// `x.`[`checked_add`]`(y).`[`unwrap_unchecked`]`()`.
        ///
        /// 如果你只是想避免 debug 模式下的 panic，**不要**使用它；
        /// 你需要的是 [`wrapping_add`]。
        ///
        /// # 安全性(Safety）
        ///
        /// 调用方必须保证加法不会溢出；当以下条件成立时调用此函数会导致 UB：
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

            // SAFETY: `unsafe` 契约要求调用方保证不会溢出；否则未检查加法会产生 UB。
            unsafe {
                intrinsics::unchecked_add(self, rhs)
            }
        }

        /// 带有符号整数的检查型加法。计算 `self + rhs`，如果发生溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".checked_add_signed(2), Some(3));")]
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".checked_add_signed(-2), None);")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).checked_add_signed(3), None);")]
        /// ```
        #[stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_add_signed(self, rhs: $SignedT) -> Option<Self> {
            let (a, b) = self.overflowing_add_signed(rhs);
            if intrinsics::unlikely(b) { None } else { Some(a) }
        }

        /// 带有符号整数的严格型加法。计算 `self + rhs`，如果发生溢出则 panic。
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
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".strict_add_signed(2), 3);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = 1", stringify!($SelfT), ".strict_add_signed(-2);")]
        /// ```
        ///
        /// ```should_panic
        #[doc = concat!("let _ = (", stringify!($SelfT), "::MAX - 2).strict_add_signed(3);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_add_signed(self, rhs: $SignedT) -> Self {
            let (a, b) = self.overflowing_add_signed(rhs);
            if b { overflow_panic::add() } else { a }
        }

        /// 检查型整数减法。计算 `self - rhs`，如果发生溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".checked_sub(1), Some(0));")]
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".checked_sub(1), None);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_checked_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
            // 根据 PR#103299，对*无符号*减法来说 `overflowing` intrinsic
            // 没有优势，我们最终还是会发出手动检查。因此，与其使用会产生
            // wrapping 减法的 `overflowing_sub`，不如自行检查，以便使用 unchecked 减法。

            if self < rhs {
                None
            } else {
                // SAFETY: 刚刚已经检查过不会溢出；未检查减法的前置条件已满足。
                Some(unsafe { intrinsics::unchecked_sub(self, rhs) })
            }
        }

        /// 严格型整数减法。计算 `self - rhs`，如果发生溢出则 panic。
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
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".strict_sub(1), 0);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = 0", stringify!($SelfT), ".strict_sub(1);")]
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

        /// 未检查整数减法。计算 `self - rhs`，并假定不会发生溢出。
        ///
        /// 调用 `x.unchecked_sub(y)` 在语义上等价于调用
        /// `x.`[`checked_sub`]`(y).`[`unwrap_unchecked`]`()`.
        ///
        /// 如果你只是想避免 debug 模式下的 panic，**不要**使用它；
        /// 你需要的是 [`wrapping_sub`]。
        ///
        /// 如果你发现自己写出了这样的代码：
        ///
        /// ```
        /// # let foo = 30_u32;
        /// # let bar = 20;
        /// if foo >= bar {
        ///     // SAFETY: 刚刚已经检查过不会溢出。
        ///     let diff = unsafe { foo.unchecked_sub(bar) };
        ///     // ... 使用 diff ...
        /// }
        /// ```
        ///
        /// 可以考虑改成：
        ///
        /// ```
        /// # let foo = 30_u32;
        /// # let bar = 20;
        /// if let Some(diff) = foo.checked_sub(bar) {
        ///     // ... 使用 diff ...
        /// }
        /// ```
        ///
        /// 这会做完全相同的事情，包括告诉优化器该减法不会溢出，
        /// 同时避免使用 `unsafe`。
        ///
        /// # 安全性(Safety）
        ///
        /// 调用方必须保证减法不会溢出；当以下条件成立时调用此函数会导致 UB：
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

            // SAFETY: `unsafe` 契约要求调用方保证不会溢出；否则未检查减法会产生 UB。
            unsafe {
                intrinsics::unchecked_sub(self, rhs)
            }
        }

        /// 带有符号整数的检查型减法。计算 `self - rhs`，如果发生溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".checked_sub_signed(2), None);")]
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".checked_sub_signed(-2), Some(3));")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).checked_sub_signed(-4), None);")]
        /// ```
        #[stable(feature = "mixed_integer_ops_unsigned_sub", since = "1.90.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops_unsigned_sub", since = "1.90.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_sub_signed(self, rhs: $SignedT) -> Option<Self> {
            let (res, overflow) = self.overflowing_sub_signed(rhs);

            if !overflow {
                Some(res)
            } else {
                None
            }
        }

        /// 带有符号整数的严格型减法。计算 `self - rhs`，如果发生溢出则 panic。
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
        #[doc = concat!("assert_eq!(3", stringify!($SelfT), ".strict_sub_signed(2), 1);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = 1", stringify!($SelfT), ".strict_sub_signed(2);")]
        /// ```
        ///
        /// ```should_panic
        #[doc = concat!("let _ = (", stringify!($SelfT), "::MAX).strict_sub_signed(-1);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn strict_sub_signed(self, rhs: $SignedT) -> Self {
            let (a, b) = self.overflowing_sub_signed(rhs);
            if b { overflow_panic::sub() } else { a }
        }

        #[doc = concat!(
            "检查型整数减法。计算 `self - rhs` 并检查结果是否能放入 [`",
            stringify!($SignedT), "`]；如果发生溢出则返回 `None`。"
        )]
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(10", stringify!($SelfT), ".checked_signed_diff(2), Some(8));")]
        #[doc = concat!("assert_eq!(2", stringify!($SelfT), ".checked_signed_diff(10), Some(-8));")]
        #[doc = concat!(
            "assert_eq!(",
            stringify!($SelfT),
            "::MAX.checked_signed_diff(",
            stringify!($SignedT),
            "::MAX as ",
            stringify!($SelfT),
            "), None);"
        )]
        #[doc = concat!(
            "assert_eq!((",
            stringify!($SignedT),
            "::MAX as ",
            stringify!($SelfT),
            ").checked_signed_diff(",
            stringify!($SelfT),
            "::MAX), Some(",
            stringify!($SignedT),
            "::MIN));"
        )]
        #[doc = concat!(
            "assert_eq!((",
            stringify!($SignedT),
            "::MAX as ",
            stringify!($SelfT),
            " + 1).checked_signed_diff(0), None);"
        )]
        #[doc = concat!(
            "assert_eq!(",
            stringify!($SelfT),
            "::MAX.checked_signed_diff(",
            stringify!($SelfT),
            "::MAX), Some(0));"
        )]
        /// ```
        #[stable(feature = "unsigned_signed_diff", since = "1.91.0")]
        #[rustc_const_stable(feature = "unsigned_signed_diff", since = "1.91.0")]
        #[inline]
        pub const fn checked_signed_diff(self, rhs: Self) -> Option<$SignedT> {
            let res = self.wrapping_sub(rhs) as $SignedT;
            let overflow = (self >= rhs) == (res < 0);

            if !overflow {
                Some(res)
            } else {
                None
            }
        }

        /// 检查型整数乘法。计算 `self * rhs`，如果发生溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".checked_mul(1), Some(5));")]
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

        /// 严格型整数乘法。计算 `self * rhs`，如果发生溢出则 panic。
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
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".strict_mul(1), 5);")]
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

        /// 未检查整数乘法。计算 `self * rhs`，并假定不会发生溢出。
        ///
        /// 调用 `x.unchecked_mul(y)` 在语义上等价于调用
        /// `x.`[`checked_mul`]`(y).`[`unwrap_unchecked`]`()`.
        ///
        /// 如果你只是想避免 debug 模式下的 panic，**不要**使用它；
        /// 你需要的是 [`wrapping_mul`]。
        ///
        /// # 安全性(Safety）
        ///
        /// 调用方必须保证乘法不会溢出；当以下条件成立时调用此函数会导致 UB：
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

            // SAFETY: `unsafe` 契约要求调用方保证不会溢出；否则未检查乘法会产生 UB。
            unsafe {
                intrinsics::unchecked_mul(self, rhs)
            }
        }

        /// 检查型整数除法。计算 `self / rhs`，如果 `rhs == 0` 则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(128", stringify!($SelfT), ".checked_div(2), Some(64));")]
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".checked_div(0), None);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_checked_int_div", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_div(self, rhs: Self) -> Option<Self> {
            if intrinsics::unlikely(rhs == 0) {
                None
            } else {
                // SAFETY: 上面已经检查除数不为零；无符号类型的除法没有其他失败模式，
                // 因此 unchecked_div 的前置条件已满足。
                Some(unsafe { intrinsics::unchecked_div(self, rhs) })
            }
        }

        /// 严格型整数除法。计算 `self / rhs`。
        ///
        /// 无符号类型上的严格型除法只是普通除法，不可能发生溢出。
        /// 此函数的存在是为了让严格型操作覆盖所有运算。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".strict_div(10), 10);")]
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
        #[inline(always)]
        #[track_caller]
        pub const fn strict_div(self, rhs: Self) -> Self {
            self / rhs
        }

        /// 检查型欧几里得除法。计算 `self.div_euclid(rhs)`，如果 `rhs == 0` 则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(128", stringify!($SelfT), ".checked_div_euclid(2), Some(64));")]
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".checked_div_euclid(0), None);")]
        /// ```
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_div_euclid(self, rhs: Self) -> Option<Self> {
            if intrinsics::unlikely(rhs == 0) {
                None
            } else {
                Some(self.div_euclid(rhs))
            }
        }

        /// 严格型欧几里得除法。计算 `self.div_euclid(rhs)`。
        ///
        /// 无符号类型上的严格型除法只是普通除法，不可能发生溢出。
        /// 此函数的存在是为了让严格型操作覆盖所有运算。由于对于正整数，
        /// 所有常见的除法定义都相同，这正好等价于 `self.strict_div(rhs)`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".strict_div_euclid(10), 10);")]
        /// ```
        /// 以下代码会因除以零而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = (1", stringify!($SelfT), ").strict_div_euclid(0);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const fn strict_div_euclid(self, rhs: Self) -> Self {
            self / rhs
        }

        /// 检查型无余数整数除法。计算 `self / rhs`，
        /// 如果 `rhs == 0` 或 `self % rhs != 0` 则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(exact_div)]
        #[doc = concat!("assert_eq!(64", stringify!($SelfT), ".checked_div_exact(2), Some(32));")]
        #[doc = concat!("assert_eq!(64", stringify!($SelfT), ".checked_div_exact(32), Some(2));")]
        #[doc = concat!("assert_eq!(64", stringify!($SelfT), ".checked_div_exact(0), None);")]
        #[doc = concat!("assert_eq!(65", stringify!($SelfT), ".checked_div_exact(2), None);")]
        /// ```
        #[unstable(
            feature = "exact_div",
            issue = "139911",
        )]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_div_exact(self, rhs: Self) -> Option<Self> {
            if intrinsics::unlikely(rhs == 0) {
                None
            } else {
                // SAFETY: 上面已经检查除数不为零；因此 unchecked_rem 的除法前置条件已满足。
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
        /// # 示例
        ///
        /// ```
        /// #![feature(exact_div)]
        #[doc = concat!("assert_eq!(64", stringify!($SelfT), ".div_exact(2), Some(32));")]
        #[doc = concat!("assert_eq!(64", stringify!($SelfT), ".div_exact(32), Some(2));")]
        #[doc = concat!("assert_eq!(65", stringify!($SelfT), ".div_exact(2), None);")]
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

        /// 未检查无余数整数除法。计算 `self / rhs`。
        ///
        /// # 安全性(Safety）
        ///
        /// 调用方必须保证 `rhs != 0` 且 `self % rhs == 0`；当 `rhs == 0`
        /// 或 `self % rhs != 0` 时调用此函数会导致 UB，也就是
        /// [`checked_div_exact`](Self::checked_div_exact) 会返回 `None` 的情况。
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
                concat!(stringify!($SelfT), "::unchecked_div_exact divide by zero or leave a remainder"),
                (
                    lhs: $SelfT = self,
                    rhs: $SelfT = rhs,
                ) => rhs > 0 && lhs % rhs == 0,
            );
            // SAFETY: `unsafe` 契约要求调用方满足同一前置条件；否则 exact_div 会产生 UB。
            unsafe { intrinsics::exact_div(self, rhs) }
        }

        /// 检查型整数取余。计算 `self % rhs`，如果 `rhs == 0` 则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".checked_rem(2), Some(1));")]
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".checked_rem(0), None);")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_checked_int_div", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_rem(self, rhs: Self) -> Option<Self> {
            if intrinsics::unlikely(rhs == 0) {
                None
            } else {
                // SAFETY: 上面已经检查除数不为零；无符号类型的取余没有其他失败模式，
                // 因此 unchecked_rem 的前置条件已满足。
                Some(unsafe { intrinsics::unchecked_rem(self, rhs) })
            }
        }

        /// 严格型整数取余。计算 `self % rhs`。
        ///
        /// 无符号类型上的严格型取余只是普通取余，不可能发生溢出。
        /// 此函数的存在是为了让严格型操作覆盖所有运算。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".strict_rem(10), 0);")]
        /// ```
        ///
        /// 以下代码会因除以零而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = 5", stringify!($SelfT), ".strict_rem(0);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const fn strict_rem(self, rhs: Self) -> Self {
            self % rhs
        }

        /// 检查型欧几里得取模。计算 `self.rem_euclid(rhs)`，如果 `rhs == 0` 则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".checked_rem_euclid(2), Some(1));")]
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".checked_rem_euclid(0), None);")]
        /// ```
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_rem_euclid(self, rhs: Self) -> Option<Self> {
            if intrinsics::unlikely(rhs == 0) {
                None
            } else {
                Some(self.rem_euclid(rhs))
            }
        }

        /// 严格型欧几里得取模。计算 `self.rem_euclid(rhs)`。
        ///
        /// 无符号类型上的严格型取模只是普通取余，不可能发生溢出。
        /// 此函数的存在是为了让严格型操作覆盖所有运算。由于对于正整数，
        /// 所有常见的除法定义都相同，这正好等价于 `self.strict_rem(rhs)`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".strict_rem_euclid(10), 0);")]
        /// ```
        ///
        /// 以下代码会因除以零而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = 5", stringify!($SelfT), ".strict_rem_euclid(0);")]
        /// ```
        #[stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[rustc_const_stable(feature = "strict_overflow_ops", since = "1.91.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const fn strict_rem_euclid(self, rhs: Self) -> Self {
            self % rhs
        }

        /// 与 `self | other` 的值相同，但如果两个输入的任意相同位位置都被置位，则为 UB。
        ///
        /// 这是一个情境性微优化：在某些地方，你可能希望根据具体指令如何与其他操作组合，
        /// 在一些平台上使用加法，在另一些平台上使用按位或。请注意，如果从涉及的操作
        /// 可以清楚看出两侧不会重叠，就没有必要使用它。例如，如果你用
        /// `((a as u32) << 16) | (b as u32)` 把 `u16` 组合成 `u32`，那就没问题；
        /// 后端会知道 `|` 两边不相交，不需要额外帮助。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(disjoint_bitor)]
        ///
        /// // SAFETY: `1` 和 `4` 没有共同置位的位。
        /// unsafe {
        #[doc = concat!("    assert_eq!(1_", stringify!($SelfT), ".unchecked_disjoint_bitor(4), 5);")]
        /// }
        /// ```
        ///
        /// # 安全性(Safety）
        ///
        /// 调用方必须保证 `(self & other) == 0`，否则会立即导致 UB。
        ///
        /// 等价地，调用方必须保证 `(self | other) == (self + other)`。
        #[unstable(feature = "disjoint_bitor", issue = "135758")]
        #[rustc_const_unstable(feature = "disjoint_bitor", issue = "135758")]
        #[inline]
        pub const unsafe fn unchecked_disjoint_bitor(self, other: Self) -> Self {
            assert_unsafe_precondition!(
                check_language_ub,
                concat!(stringify!($SelfT), "::unchecked_disjoint_bitor cannot have overlapping bits"),
                (
                    lhs: $SelfT = self,
                    rhs: $SelfT = other,
                ) => (lhs & rhs) == 0,
            );

            // SAFETY: `unsafe` 契约要求调用方满足同一前置条件；否则 disjoint_bitor 会产生 UB。
            unsafe { intrinsics::disjoint_bitor(self, other) }
        }

        /// 返回此数以任意底数为底的对数，并向下取整。
        ///
        /// 受实现细节影响，此方法可能未被优化；底数为 2 时 `ilog2` 可以更高效地产生结果，
        /// 底数为 10 时 `ilog10` 可以更高效地产生结果。
        ///
        /// # Panics
        ///
        /// 如果 `self` 为零，或 `base` 小于 2，此函数会 panic。
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

        /// 返回此数以 2 为底的对数，并向下取整。
        ///
        /// # Panics
        ///
        /// 如果 `self` 为零，此函数会 panic。
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

        /// 返回此数以 10 为底的对数，并向下取整。
        ///
        /// # Panics
        ///
        /// 如果 `self` 为零，此函数会 panic。
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

        /// 返回此数以任意底数为底的对数，并向下取整。
        ///
        /// 如果此数为零，或底数小于 2，则返回 `None`。
        ///
        /// 受实现细节影响，此方法可能未被优化；底数为 2 时 `checked_ilog2`
        /// 可以更高效地产生结果，底数为 10 时 `checked_ilog10` 可以更高效地产生结果。
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
            // 当底数在编译期已知且存在更便宜的方法时，告知编译器可以优化。
            //
            // 注意：与所有优化一样，编译器不保证一定会应用它。如果你想要这些特定底数，
            // 请直接使用 `.checked_ilog2()` 或 `.checked_ilog10()`。
            if core::intrinsics::is_val_statically_known(base) {
                if base == 2 {
                    return self.checked_ilog2();
                } else if base == 10 {
                    return self.checked_ilog10();
                }
            }

            if self <= 0 || base <= 1 {
                None
            } else if self < base {
                Some(0)
            } else {
                // 由于 self >= base，n >= 1。
                let mut n = 1;
                let mut r = base;

                // 针对 128 位宽整数的优化。
                if Self::BITS == 128 {
                    // 下面的值是 ⌊log(base,self)⌋ 的正确下界，因为
                    //
                    // log(base,self) = log(2,self) / log(2,base)
                    //                ≥ ⌊log(2,self)⌋ / (⌊log(2,base)⌋ + 1)
                    //
                    // 因此
                    //
                    // ⌊log(base,self)⌋ ≥ ⌊ ⌊log(2,self)⌋ / (⌊log(2,base)⌋ + 1) ⌋ .
                    n = self.ilog2() / (base.ilog2() + 1);
                    r = base.pow(n);
                }

                while r <= self / base {
                    n += 1;
                    r *= base;
                }
                Some(n)
            }
        }

        /// 返回此数以 2 为底的对数，并向下取整。
        ///
        /// 如果此数为零，则返回 `None`。
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
            match NonZero::new(self) {
                Some(x) => Some(x.ilog2()),
                None => None,
            }
        }

        /// 返回此数以 10 为底的对数，并向下取整。
        ///
        /// 如果此数为零，则返回 `None`。
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
            match NonZero::new(self) {
                Some(x) => Some(x.ilog10()),
                None => None,
            }
        }

        /// 检查型取负。计算 `-self`；除非 `self == 0`，否则返回 `None`。
        ///
        /// 请注意，对任何正整数取负都会溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".checked_neg(), Some(0));")]
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".checked_neg(), None);")]
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

        /// 严格型取负。计算 `-self`；除非 `self == 0`，否则 panic。
        ///
        /// 请注意，对任何正整数取负都会溢出。
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
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".strict_neg(), 0);")]
        /// ```
        ///
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = 1", stringify!($SelfT), ".strict_neg();")]
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

        /// 检查型左移。计算 `self << rhs`；如果 `rhs` 大于或等于 `self` 的位数，
        /// 则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0x1", stringify!($SelfT), ".checked_shl(4), Some(0x10));")]
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".checked_shl(129), None);")]
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".checked_shl(", stringify!($BITS_MINUS_ONE), "), Some(0));")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_checked_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_shl(self, rhs: u32) -> Option<Self> {
            // 不使用 overflowing_shl，因为那是回绕移位。
            if rhs < Self::BITS {
                // SAFETY: 刚刚已经检查 `rhs` 在范围内；unchecked_shl 的前置条件已满足。
                Some(unsafe { self.unchecked_shl(rhs) })
            } else {
                None
            }
        }

        /// 严格型左移。计算 `self << rhs`；如果 `rhs` 大于或等于 `self` 的位数，
        /// 则 panic。
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
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = 0x10", stringify!($SelfT), ".strict_shl(129);")]
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

        /// 未检查左移。计算 `self << rhs`，并假定 `rhs` 小于 `self` 的位数。
        ///
        /// # 安全性(Safety）
        ///
        /// 调用方必须保证 `rhs` 小于 `self` 的位数；如果 `rhs` 大于或等于该位数，
        /// 调用此函数会导致 UB，也就是 [`checked_shl`] 会返回 `None` 的情况。
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

            // SAFETY: `unsafe` 契约要求调用方保证 `rhs` 在范围内；否则 unchecked_shl 会产生 UB。
            unsafe {
                intrinsics::unchecked_shl(self, rhs)
            }
        }

        /// 无界左移。计算 `self << rhs`，不要求 `rhs` 受位数范围限制。
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
        ///
        #[doc = concat!("let start : ", stringify!($SelfT), " = 13;")]
        /// let mut running = start;
        /// for i in 0..160 {
        ///     // 无界左移 i 位与执行 i 次 `<< 1` 相同。
        ///     assert_eq!(running, start.unbounded_shl(i));
        ///     // 这对 wrapping 移位并不总是成立。
        #[doc = concat!("    assert_eq!(running == start.wrapping_shl(i), i < ", stringify!($BITS), ");")]
        ///
        ///     running <<= 1;
        /// }
        /// ```
        #[stable(feature = "unbounded_shifts", since = "1.87.0")]
        #[rustc_const_stable(feature = "unbounded_shifts", since = "1.87.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn unbounded_shl(self, rhs: u32) -> $SelfT{
            if rhs < Self::BITS {
                // SAFETY:
                // 上面刚刚已经检查 rhs 在范围内；否则 unchecked_shl 会产生 UB。
                unsafe { self.unchecked_shl(rhs) }
            } else {
                0
            }
        }

        /// 精确左移。只要可以无损反向还原，就计算 `self << rhs`。
        ///
        /// 如果会有任何非零位被移出，或 `rhs` >=
        #[doc = concat!("`", stringify!($SelfT), "::BITS`.")]
        /// 否则返回 `Some(self << rhs)`。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(exact_bitshifts)]
        ///
        #[doc = concat!("assert_eq!(0x1", stringify!($SelfT), ".shl_exact(4), Some(0x10));")]
        #[doc = concat!("assert_eq!(0x1", stringify!($SelfT), ".shl_exact(129), None);")]
        /// ```
        #[unstable(feature = "exact_bitshifts", issue = "144336")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn shl_exact(self, rhs: u32) -> Option<$SelfT> {
            if rhs <= self.leading_zeros() && rhs < <$SelfT>::BITS {
                // SAFETY: 上面已经检查 rhs；unchecked_shl 的前置条件已满足。
                Some(unsafe { self.unchecked_shl(rhs) })
            } else {
                None
            }
        }

        /// 未检查精确左移。计算 `self << rhs`，并假定操作可以无损反向还原，
        /// 且 `rhs` 不能大于
        #[doc = concat!("`", stringify!($SelfT), "::BITS`.")]
        ///
        /// # 安全性(Safety）
        ///
        /// 调用方必须保证移位不会移出非零位且 `rhs` 在范围内；当
        /// `rhs > self.leading_zeros() || rhs >=
        #[doc = concat!(stringify!($SelfT), "::BITS`")]
        /// 时调用此函数会导致 UB，也就是
        #[doc = concat!("[`", stringify!($SelfT), "::shl_exact`]")]
        /// 会返回 `None` 的情况。
        #[unstable(feature = "exact_bitshifts", issue = "144336")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const unsafe fn unchecked_shl_exact(self, rhs: u32) -> $SelfT {
            assert_unsafe_precondition!(
                check_library_ub,
                concat!(stringify!($SelfT), "::unchecked_shl_exact cannot shift out non-zero bits"),
                (
                    zeros: u32 = self.leading_zeros(),
                    bits: u32 =  <$SelfT>::BITS,
                    rhs: u32 = rhs,
                ) => rhs <= zeros && rhs < bits,
            );

            // SAFETY: `unsafe` 契约要求调用方满足同一前置条件；否则 unchecked_shl 会产生 UB。
            unsafe { self.unchecked_shl(rhs) }
        }

        /// 检查型右移。计算 `self >> rhs`；如果 `rhs` 大于或等于 `self` 的位数，
        /// 则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".checked_shr(4), Some(0x1));")]
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".checked_shr(129), None);")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_checked_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_shr(self, rhs: u32) -> Option<Self> {
            // 不使用 overflowing_shr，因为那是回绕移位。
            if rhs < Self::BITS {
                // SAFETY: 刚刚已经检查 `rhs` 在范围内；unchecked_shr 的前置条件已满足。
                Some(unsafe { self.unchecked_shr(rhs) })
            } else {
                None
            }
        }

        /// 严格型右移。计算 `self >> rhs`；如果 `rhs` 大于或等于 `self` 的位数，
        /// 则 panic。
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
        /// 以下代码会因溢出而 panic：
        ///
        /// ```should_panic
        #[doc = concat!("let _ = 0x10", stringify!($SelfT), ".strict_shr(129);")]
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

        /// 未检查右移。计算 `self >> rhs`，并假定 `rhs` 小于 `self` 的位数。
        ///
        /// # 安全性(Safety）
        ///
        /// 调用方必须保证 `rhs` 小于 `self` 的位数；如果 `rhs` 大于或等于该位数，
        /// 调用此函数会导致 UB，也就是 [`checked_shr`] 会返回 `None` 的情况。
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

            // SAFETY: `unsafe` 契约要求调用方保证 `rhs` 在范围内；否则 unchecked_shr 会产生 UB。
            unsafe {
                intrinsics::unchecked_shr(self, rhs)
            }
        }

        /// 无界右移。计算 `self >> rhs`，不要求 `rhs` 受位数范围限制。
        ///
        /// 如果 `rhs` 大于或等于 `self` 的位数，整个值都会被移出，并返回 `0`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0x10_", stringify!($SelfT), ".unbounded_shr(4), 0x1);")]
        #[doc = concat!("assert_eq!(0x10_", stringify!($SelfT), ".unbounded_shr(129), 0);")]
        #[doc = concat!("assert_eq!(0b1010_", stringify!($SelfT), ".unbounded_shr(0), 0b1010);")]
        #[doc = concat!("assert_eq!(0b1010_", stringify!($SelfT), ".unbounded_shr(1), 0b101);")]
        #[doc = concat!("assert_eq!(0b1010_", stringify!($SelfT), ".unbounded_shr(2), 0b10);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".unbounded_shr(", stringify!($BITS), "), 0);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".unbounded_shr(1).unbounded_shr(", stringify!($BITS_MINUS_ONE), "), 0);")]
        ///
        #[doc = concat!("let start = ", stringify!($SelfT), "::rotate_right(13, 4);")]
        /// let mut running = start;
        /// for i in 0..160 {
        ///     // 无界右移 i 位与执行 i 次 `>> 1` 相同。
        ///     assert_eq!(running, start.unbounded_shr(i));
        ///     // 这对 wrapping 移位并不总是成立。
        #[doc = concat!("    assert_eq!(running == start.wrapping_shr(i), i < ", stringify!($BITS), ");")]
        ///
        ///     running >>= 1;
        /// }
        /// ```
        #[stable(feature = "unbounded_shifts", since = "1.87.0")]
        #[rustc_const_stable(feature = "unbounded_shifts", since = "1.87.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn unbounded_shr(self, rhs: u32) -> $SelfT{
            if rhs < Self::BITS {
                // SAFETY:
                // 上面刚刚已经检查 rhs 在范围内；否则 unchecked_shr 会产生 UB。
                unsafe { self.unchecked_shr(rhs) }
            } else {
                0
            }
        }

        /// 精确右移。只要可以无损反向还原，就计算 `self >> rhs`。
        ///
        /// 如果会有任何非零位被移出，或 `rhs` >=
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
                // SAFETY: 上面已经检查 rhs；unchecked_shr 的前置条件已满足。
                Some(unsafe { self.unchecked_shr(rhs) })
            } else {
                None
            }
        }

        /// 未检查精确右移。计算 `self >> rhs`，并假定操作可以无损反向还原，
        /// 且 `rhs` 不能大于
        #[doc = concat!("`", stringify!($SelfT), "::BITS`.")]
        ///
        /// # 安全性(Safety）
        ///
        /// 调用方必须保证移位不会移出非零位且 `rhs` 在范围内；当
        /// `rhs > self.trailing_zeros() || rhs >=
        #[doc = concat!(stringify!($SelfT), "::BITS`")]
        /// 时调用此函数会导致 UB，也就是
        #[doc = concat!("[`", stringify!($SelfT), "::shr_exact`]")]
        /// 会返回 `None` 的情况。
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

            // SAFETY: `unsafe` 契约要求调用方满足同一前置条件；否则 unchecked_shr 会产生 UB。
            unsafe { self.unchecked_shr(rhs) }
        }

        /// 检查型乘方。计算 `self.pow(exp)`；如果发生溢出则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(2", stringify!($SelfT), ".checked_pow(5), Some(32));")]
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
                    // 因为 exp != 0，最终 exp 必然为 1。
                    if exp == 1 {
                        return Some(acc);
                    }
                }
                exp /= 2;
                base = try_opt!(base.checked_mul(base));
            }
        }

        /// 严格型乘方。计算 `self.pow(exp)`；如果发生溢出则 panic。
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
        #[doc = concat!("assert_eq!(2", stringify!($SelfT), ".strict_pow(5), 32);")]
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
                    // 因为 exp != 0，最终 exp 必然为 1。
                    if exp == 1 {
                        return acc;
                    }
                }
                exp /= 2;
                base = base.strict_mul(base);
            }
        }

        /// 饱和型整数加法。计算 `self + rhs`，在数值边界处饱和而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".saturating_add(1), 101);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.saturating_add(127), ", stringify!($SelfT), "::MAX);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[rustc_const_stable(feature = "const_saturating_int_methods", since = "1.47.0")]
        #[inline(always)]
        pub const fn saturating_add(self, rhs: Self) -> Self {
            intrinsics::saturating_add(self, rhs)
        }

        /// 带有符号整数的饱和型加法。计算 `self + rhs`，
        /// 在数值边界处饱和而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".saturating_add_signed(2), 3);")]
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".saturating_add_signed(-2), 0);")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).saturating_add_signed(4), ", stringify!($SelfT), "::MAX);")]
        /// ```
        #[stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn saturating_add_signed(self, rhs: $SignedT) -> Self {
            let (res, overflow) = self.overflowing_add(rhs as Self);
            if overflow == (rhs < 0) {
                res
            } else if overflow {
                Self::MAX
            } else {
                0
            }
        }

        /// 饱和型整数减法。计算 `self - rhs`，在数值边界处饱和而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".saturating_sub(27), 73);")]
        #[doc = concat!("assert_eq!(13", stringify!($SelfT), ".saturating_sub(127), 0);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[rustc_const_stable(feature = "const_saturating_int_methods", since = "1.47.0")]
        #[inline(always)]
        pub const fn saturating_sub(self, rhs: Self) -> Self {
            intrinsics::saturating_sub(self, rhs)
        }

        /// 饱和型整数减法。计算 `self` - `rhs`，在数值边界处饱和而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".saturating_sub_signed(2), 0);")]
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".saturating_sub_signed(-2), 3);")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).saturating_sub_signed(-4), ", stringify!($SelfT), "::MAX);")]
        /// ```
        #[stable(feature = "mixed_integer_ops_unsigned_sub", since = "1.90.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops_unsigned_sub", since = "1.90.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn saturating_sub_signed(self, rhs: $SignedT) -> Self {
            let (res, overflow) = self.overflowing_sub_signed(rhs);

            if !overflow {
                res
            } else if rhs < 0 {
                Self::MAX
            } else {
                0
            }
        }

        /// 饱和型整数乘法。计算 `self * rhs`，在数值边界处饱和而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(2", stringify!($SelfT), ".saturating_mul(10), 20);")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX).saturating_mul(10), ", stringify!($SelfT),"::MAX);")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_saturating_int_methods", since = "1.47.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn saturating_mul(self, rhs: Self) -> Self {
            match self.checked_mul(rhs) {
                Some(x) => x,
                None => Self::MAX,
            }
        }

        /// 饱和型整数除法。计算 `self / rhs`，在数值边界处饱和而不是溢出。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".saturating_div(2), 2);")]
        ///
        /// ```
        #[stable(feature = "saturating_div", since = "1.58.0")]
        #[rustc_const_stable(feature = "saturating_div", since = "1.58.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn saturating_div(self, rhs: Self) -> Self {
            // 在无符号类型上，整数除法不会溢出。
            self.wrapping_div(rhs)
        }

        /// 饱和型整数乘方。计算 `self.pow(exp)`，在数值边界处饱和而不是溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(4", stringify!($SelfT), ".saturating_pow(3), 64);")]
        #[doc = concat!("assert_eq!(0_", stringify!($SelfT), ".saturating_pow(0), 1);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.saturating_pow(2), ", stringify!($SelfT), "::MAX);")]
        /// ```
        #[stable(feature = "no_panic_pow", since = "1.34.0")]
        #[rustc_const_stable(feature = "const_int_pow", since = "1.50.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn saturating_pow(self, exp: u32) -> Self {
            match self.checked_pow(exp) {
                Some(x) => x,
                None => Self::MAX,
            }
        }

        /// 回绕型（模）加法。计算 `self + rhs`，在类型边界处回绕。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(200", stringify!($SelfT), ".wrapping_add(55), 255);")]
        #[doc = concat!("assert_eq!(200", stringify!($SelfT), ".wrapping_add(", stringify!($SelfT), "::MAX), 199);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_wrapping_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_add(self, rhs: Self) -> Self {
            intrinsics::wrapping_add(self, rhs)
        }

        /// 带有符号整数的回绕型（模）加法。计算 `self + rhs`，在类型边界处回绕。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".wrapping_add_signed(2), 3);")]
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".wrapping_add_signed(-2), ", stringify!($SelfT), "::MAX);")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).wrapping_add_signed(4), 1);")]
        /// ```
        #[stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn wrapping_add_signed(self, rhs: $SignedT) -> Self {
            self.wrapping_add(rhs as Self)
        }

        /// 回绕型（模）减法。计算 `self - rhs`，在类型边界处回绕。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_sub(100), 0);")]
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_sub(", stringify!($SelfT), "::MAX), 101);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_wrapping_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_sub(self, rhs: Self) -> Self {
            intrinsics::wrapping_sub(self, rhs)
        }

        /// 带有符号整数的回绕型（模）减法。计算 `self - rhs`，在类型边界处回绕。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".wrapping_sub_signed(2), ", stringify!($SelfT), "::MAX);")]
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".wrapping_sub_signed(-2), 3);")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).wrapping_sub_signed(-4), 1);")]
        /// ```
        #[stable(feature = "mixed_integer_ops_unsigned_sub", since = "1.90.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops_unsigned_sub", since = "1.90.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn wrapping_sub_signed(self, rhs: $SignedT) -> Self {
            self.wrapping_sub(rhs as Self)
        }

        /// 回绕型（模）乘法。计算 `self * rhs`，在类型边界处回绕。
        ///
        /// # 示例
        ///
        /// 请注意，此示例在多个整数类型之间共享，因此这里使用 `u8`。
        ///
        /// ```
        /// assert_eq!(10u8.wrapping_mul(12), 120);
        /// assert_eq!(25u8.wrapping_mul(12), 44);
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_wrapping_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_mul(self, rhs: Self) -> Self {
            intrinsics::wrapping_mul(self, rhs)
        }

        /// 回绕型（模）除法。计算 `self / rhs`。
        ///
        /// 无符号类型上的回绕型除法只是普通除法，不可能发生回绕。
        /// 此函数的存在是为了让回绕型操作覆盖所有运算。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_div(10), 10);")]
        /// ```
        #[stable(feature = "num_wrapping", since = "1.2.0")]
        #[rustc_const_stable(feature = "const_wrapping_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const fn wrapping_div(self, rhs: Self) -> Self {
            self / rhs
        }

        /// 回绕型欧几里得除法。计算 `self.div_euclid(rhs)`。
        ///
        /// 无符号类型上的回绕型除法只是普通除法，不可能发生回绕。
        /// 此函数的存在是为了让回绕型操作覆盖所有运算。由于对于正整数，
        /// 所有常见的除法定义都相同，这正好等价于 `self.wrapping_div(rhs)`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_div_euclid(10), 10);")]
        /// ```
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const fn wrapping_div_euclid(self, rhs: Self) -> Self {
            self / rhs
        }

        /// 回绕型（模）取余。计算 `self % rhs`。
        ///
        /// 无符号类型上的回绕型取余只是普通取余，不可能发生回绕。
        /// 此函数的存在是为了让回绕型操作覆盖所有运算。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_rem(10), 0);")]
        /// ```
        #[stable(feature = "num_wrapping", since = "1.2.0")]
        #[rustc_const_stable(feature = "const_wrapping_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const fn wrapping_rem(self, rhs: Self) -> Self {
            self % rhs
        }

        /// 回绕型欧几里得取模。计算 `self.rem_euclid(rhs)`。
        ///
        /// 无符号类型上的回绕型取模只是普通取余，不可能发生回绕。
        /// 此函数的存在是为了让回绕型操作覆盖所有运算。由于对于正整数，
        /// 所有常见的除法定义都相同，这正好等价于 `self.wrapping_rem(rhs)`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".wrapping_rem_euclid(10), 0);")]
        /// ```
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const fn wrapping_rem_euclid(self, rhs: Self) -> Self {
            self % rhs
        }

        /// 回绕型（模）取负。计算 `-self`，在类型边界处回绕。
        ///
        /// 由于无符号类型没有对应的负值，此函数的所有应用都会回绕（`-0` 除外）。
        /// 对小于对应有符号类型最大值的值，结果与转换对应有符号值相同。
        /// 更大的值等价于 `MAX + 1 - (val - MAX - 1)`，其中 `MAX`
        /// 是对应有符号类型的最大值。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0_", stringify!($SelfT), ".wrapping_neg(), 0);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.wrapping_neg(), 1);")]
        #[doc = concat!("assert_eq!(13_", stringify!($SelfT), ".wrapping_neg(), (!13) + 1);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".wrapping_neg(), !(42 - 1));")]
        /// ```
        #[stable(feature = "num_wrapping", since = "1.2.0")]
        #[rustc_const_stable(feature = "const_wrapping_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_neg(self) -> Self {
            (0 as $SelfT).wrapping_sub(self)
        }

        /// 无 panic 的按位左移；产生 `self << mask(rhs)`，
        /// 其中 `mask` 会移除 `rhs` 中会导致移位超过类型位宽的高位。
        ///
        /// 请注意，与整数上的大多数其他 `wrapping_*` 方法不同，这并不会得到
        /// “以无限精度执行移位，再按需截断”的相同结果。其行为与许多处理器上的
        /// 移位指令一致，也与禁用溢出检查时 `<<` 运算符的行为一致，
        /// 但从数值角度看比较奇怪。可以考虑改用行为更好的 [`Self::unbounded_shl`]。
        ///
        /// 请注意，这与左旋转并*不*相同；回绕左移的右侧操作数会被限制在类型范围内，
        /// 而不是把从左侧操作数移出的位送回另一端。所有基本整数类型都实现了
        /// [`rotate_left`](Self::rotate_left) 函数，它可能才是你需要的操作。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1_", stringify!($SelfT), ".wrapping_shl(7), 128);")]
        #[doc = concat!("assert_eq!(0b101_", stringify!($SelfT), ".wrapping_shl(0), 0b101);")]
        #[doc = concat!("assert_eq!(0b101_", stringify!($SelfT), ".wrapping_shl(1), 0b1010);")]
        #[doc = concat!("assert_eq!(0b101_", stringify!($SelfT), ".wrapping_shl(2), 0b10100);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.wrapping_shl(2), ", stringify!($SelfT), "::MAX - 3);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".wrapping_shl(", stringify!($BITS), "), 42);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".wrapping_shl(1).wrapping_shl(", stringify!($BITS_MINUS_ONE), "), 0);")]
        #[doc = concat!("assert_eq!(1_", stringify!($SelfT), ".wrapping_shl(128), 1);")]
        #[doc = concat!("assert_eq!(5_", stringify!($SelfT), ".wrapping_shl(1025), 10);")]
        /// ```
        #[stable(feature = "num_wrapping", since = "1.2.0")]
        #[rustc_const_stable(feature = "const_wrapping_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_shl(self, rhs: u32) -> Self {
            // SAFETY: 按类型位宽进行掩码可保证移位不会越界；否则 unchecked_shl 会产生 UB。
            unsafe {
                self.unchecked_shl(rhs & (Self::BITS - 1))
            }
        }

        /// 无 panic 的按位右移；产生 `self >> mask(rhs)`，
        /// 其中 `mask` 会移除 `rhs` 中会导致移位超过类型位宽的高位。
        ///
        /// 请注意，与整数上的大多数其他 `wrapping_*` 方法不同，这并不会得到
        /// “以无限精度执行移位，再按需截断”的相同结果。其行为与许多处理器上的
        /// 移位指令一致，也与禁用溢出检查时 `>>` 运算符的行为一致，
        /// 但从数值角度看比较奇怪。可以考虑改用行为更好的 [`Self::unbounded_shr`]。
        ///
        /// 请注意，这与右旋转并*不*相同；回绕右移的右侧操作数会被限制在类型范围内，
        /// 而不是把从左侧操作数移出的位送回另一端。所有基本整数类型都实现了
        /// [`rotate_right`](Self::rotate_right) 函数，它可能才是你需要的操作。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(128_", stringify!($SelfT), ".wrapping_shr(7), 1);")]
        #[doc = concat!("assert_eq!(0b1010_", stringify!($SelfT), ".wrapping_shr(0), 0b1010);")]
        #[doc = concat!("assert_eq!(0b1010_", stringify!($SelfT), ".wrapping_shr(1), 0b101);")]
        #[doc = concat!("assert_eq!(0b1010_", stringify!($SelfT), ".wrapping_shr(2), 0b10);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.wrapping_shr(1), ", stringify!($SignedT), "::MAX.cast_unsigned());")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".wrapping_shr(", stringify!($BITS), "), 42);")]
        #[doc = concat!("assert_eq!(42_", stringify!($SelfT), ".wrapping_shr(1).wrapping_shr(", stringify!($BITS_MINUS_ONE), "), 0);")]
        #[doc = concat!("assert_eq!(128_", stringify!($SelfT), ".wrapping_shr(128), 128);")]
        #[doc = concat!("assert_eq!(10_", stringify!($SelfT), ".wrapping_shr(1025), 5);")]
        /// ```
        #[stable(feature = "num_wrapping", since = "1.2.0")]
        #[rustc_const_stable(feature = "const_wrapping_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn wrapping_shr(self, rhs: u32) -> Self {
            // SAFETY: 按类型位宽进行掩码可保证移位不会越界；否则 unchecked_shr 会产生 UB。
            unsafe {
                self.unchecked_shr(rhs & (Self::BITS - 1))
            }
        }

        /// 回绕型（模）乘方。计算 `self.pow(exp)`，在类型边界处回绕。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(3", stringify!($SelfT), ".wrapping_pow(5), 243);")]
        /// assert_eq!(3u8.wrapping_pow(6), 217);
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

                // 因为 exp != 0，最终 exp 必然为 1。
                // 单独处理指数的最后一位，因为之后不再需要对底数平方。
                acc.wrapping_mul(base)
            } else {
                // 当指数在编译期未知时，这比上面的代码更快。常量指数场景不能使用
                // 同一段代码，因为 LLVM 当前无法展开这个循环。
                loop {
                    if (exp & 1) == 1 {
                        acc = acc.wrapping_mul(base);
                        // 因为 exp != 0，最终 exp 必然为 1。
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
        /// 如果本应发生溢出，则返回回绕后的值。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".overflowing_add(2), (7, false));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.overflowing_add(1), (0, true));")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_wrapping_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn overflowing_add(self, rhs: Self) -> (Self, bool) {
            let (a, b) = intrinsics::add_with_overflow(self as $ActualT, rhs as $ActualT);
            (a as Self, b)
        }

        /// 计算 `self` + `rhs` + `carry`，并返回包含和与输出进位的元组（按此顺序）。
        ///
        /// 对两个整数操作数和一个输入进位位执行“三元加法”，并返回一个输出整数和一个输出进位位。
        /// 这允许把多次加法串接起来以构造更宽的加法，对大整数加法很有用。
        ///
        #[doc = concat!("从电子学角度看，可以把它理解为一个 ", stringify!($BITS), " 位的“全加器”。")]
        ///
        /// 如果输入进位为 false，此方法等价于 [`overflowing_add`](Self::overflowing_add)，
        /// 且输出进位等于溢出标志。请注意，虽然对无符号整数来说进位和溢出
        /// 标志相似，但对有符号整数来说二者不同。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("//    3  MAX    (a = 3 × 2^", stringify!($BITS), " + 2^", stringify!($BITS), " - 1)")]
        #[doc = concat!("// +  5    7    (b = 5 × 2^", stringify!($BITS), " + 7)")]
        /// // ---------
        #[doc = concat!("//    9    6    (sum = 9 × 2^", stringify!($BITS), " + 6)")]
        ///
        #[doc = concat!("let (a1, a0): (", stringify!($SelfT), ", ", stringify!($SelfT), ") = (3, ", stringify!($SelfT), "::MAX);")]
        #[doc = concat!("let (b1, b0): (", stringify!($SelfT), ", ", stringify!($SelfT), ") = (5, 7);")]
        /// let carry0 = false;
        ///
        /// let (sum0, carry1) = a0.carrying_add(b0, carry0);
        /// assert_eq!(carry1, true);
        /// let (sum1, carry2) = a1.carrying_add(b1, carry1);
        /// assert_eq!(carry2, false);
        ///
        /// assert_eq!((sum1, sum0), (9, 6));
        /// ```
        #[stable(feature = "unsigned_bigint_helpers", since = "1.91.0")]
        #[rustc_const_unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn carrying_add(self, rhs: Self, carry: bool) -> (Self, bool) {
            // 注意：长期来看这应通过 intrinsic 完成，但目前已证明这样能生成最优代码，
            //   且 LLVM 没有等价的 intrinsic。
            let (a, c1) = self.overflowing_add(rhs);
            let (b, c2) = a.overflowing_add(carry as $SelfT);
            // 理想情况下，即使不告诉 LLVM，它也应知道这里不相交，
            // 但事实并非如此 <https://github.com/llvm/llvm-project/issues/118162>
            // SAFETY: `c1` 和 `c2` 至多只有一个会被置位。若 `c1` 被置位，说明已经溢出；
            // 此时 `a` 最大为 `MAX-1`，因此 `c2` 不可能再溢出，因为它最多只加 `1`
            // （该值来自 `bool`）。
            (b, unsafe { intrinsics::disjoint_bitor(c1, c2) })
        }

        /// 计算 `self` + `rhs`，其中 `rhs` 为有符号数。
        ///
        /// 返回加法结果以及一个表示是否会发生算术溢出的布尔值。
        /// 如果本应发生溢出，则返回回绕后的值。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".overflowing_add_signed(2), (3, false));")]
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".overflowing_add_signed(-2), (", stringify!($SelfT), "::MAX, true));")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).overflowing_add_signed(4), (1, true));")]
        /// ```
        #[stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops", since = "1.66.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn overflowing_add_signed(self, rhs: $SignedT) -> (Self, bool) {
            let (res, overflowed) = self.overflowing_add(rhs as Self);
            (res, overflowed ^ (rhs < 0))
        }

        /// 计算 `self - rhs`。
        ///
        /// 返回减法结果以及一个表示是否会发生算术溢出的布尔值。
        /// 如果本应发生溢出，则返回回绕后的值。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".overflowing_sub(2), (3, false));")]
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".overflowing_sub(1), (", stringify!($SelfT), "::MAX, true));")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_wrapping_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn overflowing_sub(self, rhs: Self) -> (Self, bool) {
            let (a, b) = intrinsics::sub_with_overflow(self as $ActualT, rhs as $ActualT);
            (a as Self, b)
        }

        /// 计算 `self` &minus; `rhs` &minus; `borrow`，
        /// 并返回包含差值和输出借位的元组。
        ///
        /// 通过从 `self` 中同时减去一个整数操作数和一个输入借位位来执行“三元减法”，
        /// 并返回一个输出整数和一个输出借位位。这允许把多次减法串接起来以构造更宽的减法，
        /// 对大整数减法很有用。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("//    9    6    (a = 9 × 2^", stringify!($BITS), " + 6)")]
        #[doc = concat!("// -  5    7    (b = 5 × 2^", stringify!($BITS), " + 7)")]
        /// // ---------
        #[doc = concat!("//    3  MAX    (diff = 3 × 2^", stringify!($BITS), " + 2^", stringify!($BITS), " - 1)")]
        ///
        #[doc = concat!("let (a1, a0): (", stringify!($SelfT), ", ", stringify!($SelfT), ") = (9, 6);")]
        #[doc = concat!("let (b1, b0): (", stringify!($SelfT), ", ", stringify!($SelfT), ") = (5, 7);")]
        /// let borrow0 = false;
        ///
        /// let (diff0, borrow1) = a0.borrowing_sub(b0, borrow0);
        /// assert_eq!(borrow1, true);
        /// let (diff1, borrow2) = a1.borrowing_sub(b1, borrow1);
        /// assert_eq!(borrow2, false);
        ///
        #[doc = concat!("assert_eq!((diff1, diff0), (3, ", stringify!($SelfT), "::MAX));")]
        /// ```
        #[stable(feature = "unsigned_bigint_helpers", since = "1.91.0")]
        #[rustc_const_unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn borrowing_sub(self, rhs: Self, borrow: bool) -> (Self, bool) {
            // 注意：长期来看这应通过 intrinsic 完成，但目前已证明这样能生成最优代码，
            //   且 LLVM 没有等价的 intrinsic。
            let (a, c1) = self.overflowing_sub(rhs);
            let (b, c2) = a.overflowing_sub(borrow as $SelfT);
            // SAFETY: `c1` 和 `c2` 至多只有一个会被置位。若 `c1` 被置位，说明已经下溢；
            // 此时 `a` 非零，因此 `c2` 不可能再下溢，因为它最多只减 `1`
            // （该值来自 `bool`）。
            (b, unsafe { intrinsics::disjoint_bitor(c1, c2) })
        }

        /// 计算 `self` - `rhs`，其中 `rhs` 为有符号数。
        ///
        /// 返回减法结果以及一个表示是否会发生算术溢出的布尔值。
        /// 如果本应发生溢出，则返回回绕后的值。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".overflowing_sub_signed(2), (", stringify!($SelfT), "::MAX, true));")]
        #[doc = concat!("assert_eq!(1", stringify!($SelfT), ".overflowing_sub_signed(-2), (3, false));")]
        #[doc = concat!("assert_eq!((", stringify!($SelfT), "::MAX - 2).overflowing_sub_signed(-4), (1, true));")]
        /// ```
        #[stable(feature = "mixed_integer_ops_unsigned_sub", since = "1.90.0")]
        #[rustc_const_stable(feature = "mixed_integer_ops_unsigned_sub", since = "1.90.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn overflowing_sub_signed(self, rhs: $SignedT) -> (Self, bool) {
            let (res, overflow) = self.overflowing_sub(rhs as Self);

            (res, overflow ^ (rhs < 0))
        }

        /// 计算 `self` 与 `other` 之间的绝对差。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".abs_diff(80), 20", stringify!($SelfT), ");")]
        #[doc = concat!("assert_eq!(100", stringify!($SelfT), ".abs_diff(110), 10", stringify!($SelfT), ");")]
        /// ```
        #[stable(feature = "int_abs_diff", since = "1.60.0")]
        #[rustc_const_stable(feature = "int_abs_diff", since = "1.60.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn abs_diff(self, other: Self) -> Self {
            if size_of::<Self>() == 1 {
                // 当 SSE2 可用且此函数针对 u8 自动向量化时，诱导 LLVM 生成 psadbw 指令。
                (self as i32).wrapping_sub(other as i32).unsigned_abs() as Self
            } else {
                if self < other {
                    other - self
                } else {
                    self - other
                }
            }
        }

        /// 计算 `self` 和 `rhs` 的乘积。
        ///
        /// 返回乘法结果以及一个表示是否会发生算术溢出的布尔值。
        /// 如果本应发生溢出，则返回回绕后的值。
        ///
        /// 如果你想要溢出的*值*，而不只是知道*是否*发生了溢出，
        /// 请参见 [`Self::carrying_mul`]。
        ///
        /// # 示例
        ///
        /// 请注意，此示例在多个整数类型之间共享，因此这里使用 `u32`。
        ///
        /// ```
        /// assert_eq!(5u32.overflowing_mul(2), (10, false));
        /// assert_eq!(1_000_000_000u32.overflowing_mul(10), (1410065408, true));
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_wrapping_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
        #[inline(always)]
        pub const fn overflowing_mul(self, rhs: Self) -> (Self, bool) {
            let (a, b) = intrinsics::mul_with_overflow(self as $ActualT, rhs as $ActualT);
            (a as Self, b)
        }

        /// 计算完整的双宽乘积 `self * rhs`。
        ///
        /// 这会把结果的低位（回绕）位和高位（溢出）位按顺序作为两个独立值返回。
        /// 因此，`a.widening_mul(b).0` 会产生与 `a.wrapping_mul(b)` 相同的结果。
        ///
        /// 如果还需要向宽结果加上一个值和进位，则应改用 [`Self::carrying_mul_add`]。
        ///
        /// 如果还需要向宽结果加上一个进位，则应改用 [`Self::carrying_mul`]。
        ///
        /// 如果你只是想知道乘法*是否*溢出，则应改用 [`Self::overflowing_mul`]。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(bigint_helper_methods)]
        #[doc = concat!("assert_eq!(5_", stringify!($SelfT), ".widening_mul(7), (35, 0));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.widening_mul(", stringify!($SelfT), "::MAX), (1, ", stringify!($SelfT), "::MAX - 1));")]
        /// ```
        ///
        /// 与其他 `*_mul` 方法相比：
        /// ```
        /// #![feature(bigint_helper_methods)]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::widening_mul(1 << ", stringify!($BITS_MINUS_ONE), ", 6), (0, 3));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::overflowing_mul(1 << ", stringify!($BITS_MINUS_ONE), ", 6), (0, true));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::wrapping_mul(1 << ", stringify!($BITS_MINUS_ONE), ", 6), 0);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::checked_mul(1 << ", stringify!($BITS_MINUS_ONE), ", 6), None);")]
        /// ```
        ///
        /// 请注意，此示例在多个整数类型之间共享，因此这里使用 `u32`。
        ///
        /// ```
        /// #![feature(bigint_helper_methods)]
        /// assert_eq!(5u32.widening_mul(2), (10, 0));
        /// assert_eq!(1_000_000_000u32.widening_mul(10), (1410065408, 2));
        /// ```
        #[unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[rustc_const_unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn widening_mul(self, rhs: Self) -> (Self, Self) {
            Self::carrying_mul_add(self, rhs, 0, 0)
        }

        /// 计算“完整乘法” `self * rhs + carry`，且不可能溢出。
        ///
        /// 这会把结果的低位（回绕）位和高位（溢出）位按顺序作为两个独立值返回。
        ///
        /// 执行“长乘法”：它接收一个额外要加的量，并可能返回额外的溢出量。
        /// 这允许把多次乘法串接起来，构造可表示更大值的“大整数”。
        ///
        /// 如果还需要加上一个值，请使用 [`Self::carrying_mul_add`]。
        ///
        /// # 示例
        ///
        /// 请注意，此示例在多个整数类型之间共享，因此这里使用 `u32`。
        ///
        /// ```
        /// assert_eq!(5u32.carrying_mul(2, 0), (10, 0));
        /// assert_eq!(5u32.carrying_mul(2, 10), (20, 0));
        /// assert_eq!(1_000_000_000u32.carrying_mul(10, 0), (1410065408, 2));
        /// assert_eq!(1_000_000_000u32.carrying_mul(10, 10), (1410065418, 2));
        #[doc = concat!("assert_eq!(",
            stringify!($SelfT), "::MAX.carrying_mul(", stringify!($SelfT), "::MAX, ", stringify!($SelfT), "::MAX), ",
            "(0, ", stringify!($SelfT), "::MAX));"
        )]
        /// ```
        ///
        /// 为宽于原生字长的类型实现标量乘法时，这是所需的核心操作。
        ///
        /// ```
        /// #![feature(bigint_helper_methods)]
        /// fn scalar_mul_eq(little_endian_digits: &mut Vec<u16>, multiplicand: u16) {
        ///     let mut carry = 0;
        ///     for d in little_endian_digits.iter_mut() {
        ///         (*d, carry) = d.carrying_mul(multiplicand, carry);
        ///     }
        ///     if carry != 0 {
        ///         little_endian_digits.push(carry);
        ///     }
        /// }
        ///
        /// let mut v = vec![10, 20];
        /// scalar_mul_eq(&mut v, 3);
        /// assert_eq!(v, [30, 60]);
        ///
        /// assert_eq!(0x87654321_u64 * 0xFEED, 0x86D3D159E38D);
        /// let mut v = vec![0x4321, 0x8765];
        /// scalar_mul_eq(&mut v, 0xFEED);
        /// assert_eq!(v, [0xE38D, 0xD159, 0x86D3]);
        /// ```
        ///
        /// 如果 `carry` 为零，这类似于 [`overflowing_mul`](Self::overflowing_mul)，
        /// 但它会给出溢出的值，而不只是说明是否发生了溢出：
        ///
        /// ```
        /// #![feature(bigint_helper_methods)]
        /// let r = u8::carrying_mul(7, 13, 0);
        /// assert_eq!((r.0, r.1 != 0), u8::overflowing_mul(7, 13));
        /// let r = u8::carrying_mul(13, 42, 0);
        /// assert_eq!((r.0, r.1 != 0), u8::overflowing_mul(13, 42));
        /// ```
        ///
        /// 返回元组第一个字段的值与组合使用 [`wrapping_mul`](Self::wrapping_mul)
        /// 和 [`wrapping_add`](Self::wrapping_add) 方法得到的值一致：
        ///
        /// ```
        /// #![feature(bigint_helper_methods)]
        /// assert_eq!(
        ///     789_u16.carrying_mul(456, 123).0,
        ///     789_u16.wrapping_mul(456).wrapping_add(123),
        /// );
        /// ```
        #[stable(feature = "unsigned_bigint_helpers", since = "1.91.0")]
        #[rustc_const_unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn carrying_mul(self, rhs: Self, carry: Self) -> (Self, Self) {
            Self::carrying_mul_add(self, rhs, carry, 0)
        }

        /// 计算“完整乘法” `self * rhs + carry + add`。
        ///
        /// 这会把结果的低位（回绕）位和高位（溢出）位按顺序作为两个独立值返回。
        ///
        /// 这不可能溢出，因为双宽结果恰好有足够空间容纳最大可能结果。这等价于十进制中的
        /// 9 × 9 + 9 + 9 = 81 + 18 = 99 = 9×10⁰ + 9×10¹ = 10² - 1。
        ///
        /// 执行“长乘法”：它接收一个额外要加的量，并可能返回额外的溢出量。
        /// 这允许把多次乘法串接起来，构造可表示更大值的“大整数”。
        ///
        /// 如果不需要 `add` 部分，可以改用 [`Self::carrying_mul`]。
        ///
        /// # 示例
        ///
        /// 请注意，此示例在多个整数类型之间共享，因此这里使用 `u32`。
        ///
        /// ```
        /// assert_eq!(5u32.carrying_mul_add(2, 0, 0), (10, 0));
        /// assert_eq!(5u32.carrying_mul_add(2, 10, 10), (30, 0));
        /// assert_eq!(1_000_000_000u32.carrying_mul_add(10, 0, 0), (1410065408, 2));
        /// assert_eq!(1_000_000_000u32.carrying_mul_add(10, 10, 10), (1410065428, 2));
        #[doc = concat!("assert_eq!(",
            stringify!($SelfT), "::MAX.carrying_mul_add(", stringify!($SelfT), "::MAX, ", stringify!($SelfT), "::MAX, ", stringify!($SelfT), "::MAX), ",
            "(", stringify!($SelfT), "::MAX, ", stringify!($SelfT), "::MAX));"
        )]
        /// ```
        ///
        /// 这是“竖式” O(n²) 乘法的逐位核心操作。
        ///
        /// 请注意，此示例在多个整数类型之间共享；为便于演示，这里使用 `u8`。
        ///
        /// ```
        /// fn quadratic_mul<const N: usize>(a: [u8; N], b: [u8; N]) -> [u8; N] {
        ///     let mut out = [0; N];
        ///     for j in 0..N {
        ///         let mut carry = 0;
        ///         for i in 0..(N - j) {
        ///             (out[j + i], carry) = u8::carrying_mul_add(a[i], b[j], out[j + i], carry);
        ///         }
        ///     }
        ///     out
        /// }
        ///
        /// // -1 * -1 == 1
        /// assert_eq!(quadratic_mul([0xFF; 3], [0xFF; 3]), [1, 0, 0]);
        ///
        /// assert_eq!(u32::wrapping_mul(0x9e3779b9, 0x7f4a7c15), 0xcffc982d);
        /// assert_eq!(
        ///     quadratic_mul(u32::to_le_bytes(0x9e3779b9), u32::to_le_bytes(0x7f4a7c15)),
        ///     u32::to_le_bytes(0xcffc982d)
        /// );
        /// ```
        #[stable(feature = "unsigned_bigint_helpers", since = "1.91.0")]
        #[rustc_const_unstable(feature = "bigint_helper_methods", issue = "85532")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn carrying_mul_add(self, rhs: Self, carry: Self, add: Self) -> (Self, Self) {
            intrinsics::carrying_mul_add(self, rhs, carry, add)
        }

        /// 计算 `self` 除以 `rhs` 时的商。
        ///
        /// 返回商以及一个表示是否会发生算术溢出的布尔值。请注意，
        /// 对无符号整数来说永远不会发生溢出，因此第二个值始终为 `false`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".overflowing_div(2), (2, false));")]
        /// ```
        #[inline(always)]
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_overflowing_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[track_caller]
        pub const fn overflowing_div(self, rhs: Self) -> (Self, bool) {
            (self / rhs, false)
        }

        /// 计算欧几里得除法 `self.div_euclid(rhs)` 的商。
        ///
        /// 返回商以及一个表示是否会发生算术溢出的布尔值。请注意，
        /// 对无符号整数来说永远不会发生溢出，因此第二个值始终为 `false`。
        /// 由于对于正整数，所有常见的除法定义都相同，这正好等价于
        /// `self.overflowing_div(rhs)`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".overflowing_div_euclid(2), (2, false));")]
        /// ```
        #[inline(always)]
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[track_caller]
        pub const fn overflowing_div_euclid(self, rhs: Self) -> (Self, bool) {
            (self / rhs, false)
        }

        /// 计算 `self` 除以 `rhs` 时的余数。
        ///
        /// 返回除法后的余数以及一个表示是否会发生算术溢出的布尔值。请注意，
        /// 对无符号整数来说永远不会发生溢出，因此第二个值始终为 `false`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".overflowing_rem(2), (1, false));")]
        /// ```
        #[inline(always)]
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_overflowing_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[track_caller]
        pub const fn overflowing_rem(self, rhs: Self) -> (Self, bool) {
            (self % rhs, false)
        }

        /// 像执行欧几里得除法一样计算余数 `self.rem_euclid(rhs)`。
        ///
        /// 返回除法后的模以及一个表示是否会发生算术溢出的布尔值。请注意，
        /// 对无符号整数来说永远不会发生溢出，因此第二个值始终为 `false`。
        /// 由于对于正整数，所有常见的除法定义都相同，此操作正好等价于
        /// `self.overflowing_rem(rhs)`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(5", stringify!($SelfT), ".overflowing_rem_euclid(2), (1, false));")]
        /// ```
        #[inline(always)]
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[track_caller]
        pub const fn overflowing_rem_euclid(self, rhs: Self) -> (Self, bool) {
            (self % rhs, false)
        }

        /// 以 overflowing 语义对 `self` 取负。
        ///
        /// 使用 wrapping 运算返回 `!self + 1`，也就是此无符号值在补码意义下的相反数。
        /// 注意，对正的无符号值取负总会报告溢出；只有对 `0` 取负不会溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".overflowing_neg(), (0, false));")]
        #[doc = concat!("assert_eq!(2", stringify!($SelfT), ".overflowing_neg(), (-2i32 as ", stringify!($SelfT), ", true));")]
        /// ```
        #[inline(always)]
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_wrapping_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        pub const fn overflowing_neg(self) -> (Self, bool) {
            ((!self).wrapping_add(1), self != 0)
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
        #[doc = concat!("assert_eq!(0x1", stringify!($SelfT), ".overflowing_shl(4), (0x10, false));")]
        #[doc = concat!("assert_eq!(0x1", stringify!($SelfT), ".overflowing_shl(132), (0x10, true));")]
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".overflowing_shl(", stringify!($BITS_MINUS_ONE), "), (0, false));")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_wrapping_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
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
        #[doc = concat!("assert_eq!(0x10", stringify!($SelfT), ".overflowing_shr(132), (0x1, true));")]
        /// ```
        #[stable(feature = "wrapping", since = "1.7.0")]
        #[rustc_const_stable(feature = "const_wrapping_math", since = "1.32.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn overflowing_shr(self, rhs: u32) -> (Self, bool) {
            (self.wrapping_shr(rhs), rhs >= Self::BITS)
        }

        /// 使用平方求幂计算 `self` 的 `exp` 次方。
        ///
        /// 返回幂运算结果和一个布尔值；该布尔值表示计算过程中是否发生过溢出。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(3", stringify!($SelfT), ".overflowing_pow(5), (243, false));")]
        #[doc = concat!("assert_eq!(0_", stringify!($SelfT), ".overflowing_pow(0), (1, false));")]
        /// assert_eq!(3u8.overflowing_pow(6), (217, true));
        /// ```
        #[stable(feature = "no_panic_pow", since = "1.34.0")]
        #[rustc_const_stable(feature = "const_int_pow", since = "1.50.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn overflowing_pow(self, mut exp: u32) -> (Self, bool) {
            if exp == 0{
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
        #[doc = concat!("assert_eq!(2", stringify!($SelfT), ".pow(5), 32);")]
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
        pub const fn isqrt(self) -> Self {
            let result = crate::num::int_sqrt::$ActualT(self as $ActualT) as $SelfT;

            // 告诉优化器输出范围是什么。如果测试 `core` 时没有 panic 消息就崩溃，
            // 并且某个 `num::int_sqrt::u*` 测试失败，通常是因为你的改动让这些断言
            // 或 `nonzero.rs` 的 `fn isqrt` 中的断言变成了 false。
            //
            // SAFETY: 整数平方根是单调非递减函数，也就是说增大输入不会让输出变小。
            // 因此，无符号整数输入被 `[0, <$ActualT>::MAX]` 限定时，`sqrt(n)`
            // 也一定被 `[sqrt(0), sqrt(<$ActualT>::MAX)]` 限定；这里向优化器承诺的
            // 上界与真实数学范围一致。
            unsafe {
                const MAX_RESULT: $SelfT = crate::num::int_sqrt::$ActualT(<$ActualT>::MAX) as $SelfT;
                crate::hint::assert_unchecked(result <= MAX_RESULT);
            }

            result
        }

        /// 执行 Euclidean 除法。
        ///
        /// 对正整数来说，常见的除法定义都会得到相同结果，因此这里完全等同于 `self / rhs`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(7", stringify!($SelfT), ".div_euclid(4), 1); // or any other integer type")]
        /// ```
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const fn div_euclid(self, rhs: Self) -> Self {
            self / rhs
        }


        /// 计算 `self` 除以 `rhs` 后的最小非负余数。
        ///
        /// 对正整数来说，常见的除法定义都会得到相同结果，因此这里完全等同于 `self % rhs`。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(7", stringify!($SelfT), ".rem_euclid(4), 3); // or any other integer type")]
        /// ```
        #[doc(alias = "modulo", alias = "mod")]
        #[stable(feature = "euclidean_division", since = "1.38.0")]
        #[rustc_const_stable(feature = "const_euclidean_int_methods", since = "1.52.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const fn rem_euclid(self, rhs: Self) -> Self {
            self % rhs
        }

        /// 计算 `self` 与 `rhs` 的商，并将结果向负无穷舍入。
        ///
        /// 对所有无符号整数来说，这与执行 `self / rhs` 相同。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(int_roundings)]
        #[doc = concat!("assert_eq!(7_", stringify!($SelfT), ".div_floor(4), 1);")]
        /// ```
        #[unstable(feature = "int_roundings", issue = "88581")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        #[track_caller]
        pub const fn div_floor(self, rhs: Self) -> Self {
            self / rhs
        }

        /// 计算 `self` 与 `rhs` 的商，并将结果向正无穷舍入。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(7_", stringify!($SelfT), ".div_ceil(4), 2);")]
        /// ```
        #[stable(feature = "int_roundings1", since = "1.73.0")]
        #[rustc_const_stable(feature = "int_roundings1", since = "1.73.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[track_caller]
        pub const fn div_ceil(self, rhs: Self) -> Self {
            let d = self / rhs;
            let r = self % rhs;
            if r > 0 {
                d + 1
            } else {
                d
            }
        }

        /// 计算大于或等于 `self` 且为 `rhs` 倍数的最小值。
        ///
        /// # Panics
        ///
        /// 如果 `rhs` 为零，此函数会 panic。
        ///
        /// ## 溢出行为
        ///
        /// 发生溢出时，如果启用了溢出检查（debug 模式默认启用），此函数会 panic；
        /// 如果禁用了溢出检查（release 模式默认禁用），则会 wrap。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(16_", stringify!($SelfT), ".next_multiple_of(8), 16);")]
        #[doc = concat!("assert_eq!(23_", stringify!($SelfT), ".next_multiple_of(8), 24);")]
        /// ```
        #[stable(feature = "int_roundings1", since = "1.73.0")]
        #[rustc_const_stable(feature = "int_roundings1", since = "1.73.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[rustc_inherit_overflow_checks]
        pub const fn next_multiple_of(self, rhs: Self) -> Self {
            match self % rhs {
                0 => self,
                r => self + (rhs - r)
            }
        }

        /// 计算大于或等于 `self` 且为 `rhs` 倍数的最小值。
        /// 如果 `rhs` 为零或操作会导致溢出，则返回 `None`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(16_", stringify!($SelfT), ".checked_next_multiple_of(8), Some(16));")]
        #[doc = concat!("assert_eq!(23_", stringify!($SelfT), ".checked_next_multiple_of(8), Some(24));")]
        #[doc = concat!("assert_eq!(1_", stringify!($SelfT), ".checked_next_multiple_of(0), None);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.checked_next_multiple_of(2), None);")]
        /// ```
        #[stable(feature = "int_roundings1", since = "1.73.0")]
        #[rustc_const_stable(feature = "int_roundings1", since = "1.73.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_next_multiple_of(self, rhs: Self) -> Option<Self> {
            match try_opt!(self.checked_rem(rhs)) {
                0 => Some(self),
                // rhs - r 不会溢出，因为 r 小于 rhs。
                r => self.checked_add(rhs - r)
            }
        }

        /// 如果 `self` 是 `rhs` 的整数倍，则返回 `true`；否则返回 false。
        ///
        /// 此函数等价于 `self % rhs == 0`，但在 `rhs == 0` 时不会 panic。
        /// 此时 `0.is_multiple_of(0) == true`，而对任何非零 `n`，
        /// `n.is_multiple_of(0) == false`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert!(6_", stringify!($SelfT), ".is_multiple_of(2));")]
        #[doc = concat!("assert!(!5_", stringify!($SelfT), ".is_multiple_of(2));")]
        ///
        #[doc = concat!("assert!(0_", stringify!($SelfT), ".is_multiple_of(0));")]
        #[doc = concat!("assert!(!6_", stringify!($SelfT), ".is_multiple_of(0));")]
        /// ```
        #[stable(feature = "unsigned_is_multiple_of", since = "1.87.0")]
        #[rustc_const_stable(feature = "unsigned_is_multiple_of", since = "1.87.0")]
        #[must_use]
        #[inline]
        pub const fn is_multiple_of(self, rhs: Self) -> bool {
            match rhs {
                0 => self == 0,
                _ => self % rhs == 0,
            }
        }

        /// 当且仅当某个无符号整数 `k` 满足 `self == 2^k` 时，返回 `true`。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert!(16", stringify!($SelfT), ".is_power_of_two());")]
        #[doc = concat!("assert!(!10", stringify!($SelfT), ".is_power_of_two());")]
        /// ```
        #[must_use]
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_is_power_of_two", since = "1.32.0")]
        #[inline(always)]
        pub const fn is_power_of_two(self) -> bool {
            self.count_ones() == 1
        }

        // 返回比下一个 2 的幂小 1 的值。
        // （对 8u8 来说，下一个 2 的幂是 8u8；对 6u8 来说则是 8u8）
        //
        // 8u8.one_less_than_next_power_of_two() == 7
        // 6u8.one_less_than_next_power_of_two() == 7
        //
        // 此方法不会溢出，因为在 `next_power_of_two` 会溢出的情况下，
        // 它最终会返回该类型的最大值；对 0 则可以返回 0。
        #[inline]
        const fn one_less_than_next_power_of_two(self) -> Self {
            if self <= 1 { return 0; }

            let p = self - 1;
            // SAFETY: 因为 `p > 0`，它不可能全是前导零；这意味着移位始终在范围内。
            // 某些处理器（如 intel pre-haswell）在实参非零时有更高效的 ctlz intrinsic。
            let z = unsafe { intrinsics::ctlz_nonzero(p) };
            <$SelfT>::MAX >> z
        }

        /// 返回大于或等于 `self` 的最小 2 的幂。
        ///
        /// 当返回值会溢出时（即对类型 `uN` 有 `self > (1 << (N-1))`），
        /// 它会在 debug 模式下 panic，并在 release 模式下把返回值 wrap 为 0
        /// （这是此方法唯一会返回 0 的情况）。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(2", stringify!($SelfT), ".next_power_of_two(), 2);")]
        #[doc = concat!("assert_eq!(3", stringify!($SelfT), ".next_power_of_two(), 4);")]
        #[doc = concat!("assert_eq!(0", stringify!($SelfT), ".next_power_of_two(), 1);")]
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_pow", since = "1.50.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        #[rustc_inherit_overflow_checks]
        pub const fn next_power_of_two(self) -> Self {
            self.one_less_than_next_power_of_two() + 1
        }

        /// 返回大于或等于 `self` 的最小 2 的幂。如果下一个 2 的幂大于此类型最大值，
        /// 则返回 `None`；否则把该 2 的幂包装在 `Some` 中返回。
        ///
        /// # 示例
        ///
        /// ```
        #[doc = concat!("assert_eq!(2", stringify!($SelfT), ".checked_next_power_of_two(), Some(2));")]
        #[doc = concat!("assert_eq!(3", stringify!($SelfT), ".checked_next_power_of_two(), Some(4));")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.checked_next_power_of_two(), None);")]
        /// ```
        #[inline]
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_stable(feature = "const_int_pow", since = "1.50.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        pub const fn checked_next_power_of_two(self) -> Option<Self> {
            self.one_less_than_next_power_of_two().checked_add(1)
        }

        /// 返回大于或等于 `n` 的最小 2 的幂。如果下一个 2 的幂大于此类型最大值，
        /// 则返回值会 wrap 为 `0`。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(wrapping_next_power_of_two)]
        ///
        #[doc = concat!("assert_eq!(2", stringify!($SelfT), ".wrapping_next_power_of_two(), 2);")]
        #[doc = concat!("assert_eq!(3", stringify!($SelfT), ".wrapping_next_power_of_two(), 4);")]
        #[doc = concat!("assert_eq!(", stringify!($SelfT), "::MAX.wrapping_next_power_of_two(), 0);")]
        /// ```
        #[inline]
        #[unstable(feature = "wrapping_next_power_of_two", issue = "32463",
                   reason = "needs decision on wrapping behavior")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        pub const fn wrapping_next_power_of_two(self) -> Self {
            self.one_less_than_next_power_of_two().wrapping_add(1)
        }

        /// 以 big-endian（网络）字节序的字节数组返回此整数的内存表示。
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

        /// 以 little-endian 字节序的字节数组返回此整数的内存表示。
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

        /// 以原生字节序的字节数组返回此整数的内存表示。
        ///
        /// 由于使用目标平台的原生字节序，可移植代码应酌情改用
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
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[allow(unnecessary_transmutes)]
        // SAFETY: 这个 const 转换是健全的，因为整数是 plain old datatype，
        // 所有位模式都有效；与同大小字节数组之间 transmute 只是在保留大小的前提下
        // 重解释内存表示。
        #[inline]
        pub const fn to_ne_bytes(self) -> [u8; size_of::<Self>()] {
            // SAFETY: 整数所有位模式都有效，且 `[u8; size_of::<Self>()]` 与 `Self` 大小相同；
            // transmute 到字节数组只暴露其内存表示，不会产生无效值。
            unsafe { mem::transmute(self) }
        }

        /// 从 big endian 字节数组表示创建一个原生字节序整数值。
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

        /// 从 little endian 字节数组表示创建一个原生字节序整数值。
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

        /// 从原生字节序字节数组形式的内存表示创建一个原生字节序整数值。
        ///
        /// 由于使用目标平台的原生字节序，可移植代码通常应酌情改用
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
        #[doc = concat!("    ", $be_bytes, "")]
        /// } else {
        #[doc = concat!("    ", $le_bytes, "")]
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
        // 所有位模式都有效；从同大小字节数组 transmute 到整数不会构造无效整数值。
        #[inline]
        pub const fn from_ne_bytes(bytes: [u8; size_of::<Self>()]) -> Self {
            // SAFETY: 整数所有位模式都有效，且 `[u8; size_of::<Self>()]` 与 `Self` 大小相同；
            // 从字节数组 transmute 到整数只是在原生字节序下重解释内存表示。
            unsafe { mem::transmute(bytes) }
        }

        /// 新代码应优先使用
        #[doc = concat!("[`", stringify!($SelfT), "::MIN", "`]。")]
        ///
        /// 返回此整数类型可以表示的最小值。
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_promotable]
        #[inline(always)]
        #[rustc_const_stable(feature = "const_max_value", since = "1.32.0")]
        #[deprecated(since = "TBD", note = "replaced by the `MIN` associated constant on this type")]
        #[rustc_diagnostic_item = concat!(stringify!($SelfT), "_legacy_fn_min_value")]
        pub const fn min_value() -> Self { Self::MIN }

        /// 新代码应优先使用
        #[doc = concat!("[`", stringify!($SelfT), "::MAX", "`]。")]
        ///
        /// 返回此整数类型可以表示的最大值。
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_promotable]
        #[inline(always)]
        #[rustc_const_stable(feature = "const_max_value", since = "1.32.0")]
        #[deprecated(since = "TBD", note = "replaced by the `MAX` associated constant on this type")]
        #[rustc_diagnostic_item = concat!(stringify!($SelfT), "_legacy_fn_max_value")]
        pub const fn max_value() -> Self { Self::MAX }
    }
}
