/// 一元逻辑取反运算符 `!`。
///
/// # 示例
///
/// 为 `Answer` 实现 `Not`,从而允许用 `!` 翻转它的值。
///
/// ```
/// use std::ops::Not;
///
/// #[derive(Debug, PartialEq)]
/// enum Answer {
///     Yes,
///     No,
/// }
///
/// impl Not for Answer {
///     type Output = Self;
///
///     fn not(self) -> Self::Output {
///         match self {
///             Answer::Yes => Answer::No,
///             Answer::No => Answer::Yes
///         }
///     }
/// }
///
/// assert_eq!(!Answer::Yes, Answer::No);
/// assert_eq!(!Answer::No, Answer::Yes);
/// ```
#[lang = "not"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[doc(alias = "!")]
pub const trait Not {
    /// 应用 `!` 运算符之后得到的结果类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// 执行一元 `!` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(!true, false);
    /// assert_eq!(!false, true);
    /// assert_eq!(!1u8, 254);
    /// assert_eq!(!0u8, 255);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn not(self) -> Self::Output;
}

macro_rules! not_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Not for $t {
            type Output = $t;

            #[inline]
            fn not(self) -> $t { !self }
        }

        forward_ref_unop! { impl Not, not for $t,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )*)
}

not_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

#[stable(feature = "not_never", since = "1.60.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const Not for ! {
    type Output = !;

    #[inline]
    fn not(self) -> ! {
        match self {}
    }
}

/// 按位与运算符 `&`。
///
/// 注意 `Rhs` 默认是 `Self`,但这并非强制。
///
/// # 示例
///
/// 为一个 `bool` 的包装类型实现 `BitAnd`。
///
/// ```
/// use std::ops::BitAnd;
///
/// #[derive(Debug, PartialEq)]
/// struct Scalar(bool);
///
/// impl BitAnd for Scalar {
///     type Output = Self;
///
///     // rhs 是表达式 `a & b` 的“右操作数”(right-hand side)
///     fn bitand(self, rhs: Self) -> Self::Output {
///         Self(self.0 & rhs.0)
///     }
/// }
///
/// assert_eq!(Scalar(true) & Scalar(true), Scalar(true));
/// assert_eq!(Scalar(true) & Scalar(false), Scalar(false));
/// assert_eq!(Scalar(false) & Scalar(true), Scalar(false));
/// assert_eq!(Scalar(false) & Scalar(false), Scalar(false));
/// ```
///
/// 为一个 `Vec<bool>` 的包装类型实现 `BitAnd`。
///
/// ```
/// use std::ops::BitAnd;
///
/// #[derive(Debug, PartialEq)]
/// struct BooleanVector(Vec<bool>);
///
/// impl BitAnd for BooleanVector {
///     type Output = Self;
///
///     fn bitand(self, Self(rhs): Self) -> Self::Output {
///         let Self(lhs) = self;
///         assert_eq!(lhs.len(), rhs.len());
///         Self(
///             lhs.iter()
///                 .zip(rhs.iter())
///                 .map(|(x, y)| *x & *y)
///                 .collect()
///         )
///     }
/// }
///
/// let bv1 = BooleanVector(vec![true, true, false, false]);
/// let bv2 = BooleanVector(vec![true, false, true, false]);
/// let expected = BooleanVector(vec![true, false, false, false]);
/// assert_eq!(bv1 & bv2, expected);
/// ```
#[lang = "bitand"]
#[doc(alias = "&")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "no implementation for `{Self} & {Rhs}`",
    label = "no implementation for `{Self} & {Rhs}`"
)]
pub const trait BitAnd<Rhs = Self> {
    /// 应用 `&` 运算符之后得到的结果类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// 执行 `&` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(true & false, false);
    /// assert_eq!(true & true, true);
    /// assert_eq!(5u8 & 1u8, 1);
    /// assert_eq!(5u8 & 2u8, 0);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn bitand(self, rhs: Rhs) -> Self::Output;
}

macro_rules! bitand_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitAnd for $t {
            type Output = $t;

            #[inline]
            fn bitand(self, rhs: $t) -> $t { self & rhs }
        }

        forward_ref_binop! { impl BitAnd, bitand for $t, $t,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )*)
}

bitand_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// 按位或运算符 `|`。
///
/// 注意 `Rhs` 默认是 `Self`,但这并非强制。
///
/// # 示例
///
/// 为一个 `bool` 的包装类型实现 `BitOr`。
///
/// ```
/// use std::ops::BitOr;
///
/// #[derive(Debug, PartialEq)]
/// struct Scalar(bool);
///
/// impl BitOr for Scalar {
///     type Output = Self;
///
///     // rhs 是表达式 `a | b` 的“右操作数”(right-hand side)
///     fn bitor(self, rhs: Self) -> Self::Output {
///         Self(self.0 | rhs.0)
///     }
/// }
///
/// assert_eq!(Scalar(true) | Scalar(true), Scalar(true));
/// assert_eq!(Scalar(true) | Scalar(false), Scalar(true));
/// assert_eq!(Scalar(false) | Scalar(true), Scalar(true));
/// assert_eq!(Scalar(false) | Scalar(false), Scalar(false));
/// ```
///
/// 为一个 `Vec<bool>` 的包装类型实现 `BitOr`。
///
/// ```
/// use std::ops::BitOr;
///
/// #[derive(Debug, PartialEq)]
/// struct BooleanVector(Vec<bool>);
///
/// impl BitOr for BooleanVector {
///     type Output = Self;
///
///     fn bitor(self, Self(rhs): Self) -> Self::Output {
///         let Self(lhs) = self;
///         assert_eq!(lhs.len(), rhs.len());
///         Self(
///             lhs.iter()
///                 .zip(rhs.iter())
///                 .map(|(x, y)| *x | *y)
///                 .collect()
///         )
///     }
/// }
///
/// let bv1 = BooleanVector(vec![true, true, false, false]);
/// let bv2 = BooleanVector(vec![true, false, true, false]);
/// let expected = BooleanVector(vec![true, true, true, false]);
/// assert_eq!(bv1 | bv2, expected);
/// ```
#[lang = "bitor"]
#[doc(alias = "|")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "no implementation for `{Self} | {Rhs}`",
    label = "no implementation for `{Self} | {Rhs}`"
)]
pub const trait BitOr<Rhs = Self> {
    /// 应用 `|` 运算符之后得到的结果类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// 执行 `|` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(true | false, true);
    /// assert_eq!(false | false, false);
    /// assert_eq!(5u8 | 1u8, 5);
    /// assert_eq!(5u8 | 2u8, 7);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn bitor(self, rhs: Rhs) -> Self::Output;
}

macro_rules! bitor_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitOr for $t {
            type Output = $t;

            #[inline]
            fn bitor(self, rhs: $t) -> $t { self | rhs }
        }

        forward_ref_binop! { impl BitOr, bitor for $t, $t,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )*)
}

bitor_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// 按位异或运算符 `^`。
///
/// 注意 `Rhs` 默认是 `Self`,但这并非强制。
///
/// # 示例
///
/// 一个把 `^` 提升到 `bool` 包装类型上的 `BitXor` 实现。
///
/// ```
/// use std::ops::BitXor;
///
/// #[derive(Debug, PartialEq)]
/// struct Scalar(bool);
///
/// impl BitXor for Scalar {
///     type Output = Self;
///
///     // rhs 是表达式 `a ^ b` 的“右操作数”(right-hand side)
///     fn bitxor(self, rhs: Self) -> Self::Output {
///         Self(self.0 ^ rhs.0)
///     }
/// }
///
/// assert_eq!(Scalar(true) ^ Scalar(true), Scalar(false));
/// assert_eq!(Scalar(true) ^ Scalar(false), Scalar(true));
/// assert_eq!(Scalar(false) ^ Scalar(true), Scalar(true));
/// assert_eq!(Scalar(false) ^ Scalar(false), Scalar(false));
/// ```
///
/// 为一个 `Vec<bool>` 的包装类型实现 `BitXor` trait。
///
/// ```
/// use std::ops::BitXor;
///
/// #[derive(Debug, PartialEq)]
/// struct BooleanVector(Vec<bool>);
///
/// impl BitXor for BooleanVector {
///     type Output = Self;
///
///     fn bitxor(self, Self(rhs): Self) -> Self::Output {
///         let Self(lhs) = self;
///         assert_eq!(lhs.len(), rhs.len());
///         Self(
///             lhs.iter()
///                 .zip(rhs.iter())
///                 .map(|(x, y)| *x ^ *y)
///                 .collect()
///         )
///     }
/// }
///
/// let bv1 = BooleanVector(vec![true, true, false, false]);
/// let bv2 = BooleanVector(vec![true, false, true, false]);
/// let expected = BooleanVector(vec![false, true, true, false]);
/// assert_eq!(bv1 ^ bv2, expected);
/// ```
#[lang = "bitxor"]
#[doc(alias = "^")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "no implementation for `{Self} ^ {Rhs}`",
    label = "no implementation for `{Self} ^ {Rhs}`"
)]
pub const trait BitXor<Rhs = Self> {
    /// 应用 `^` 运算符之后得到的结果类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// 执行 `^` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(true ^ false, true);
    /// assert_eq!(true ^ true, false);
    /// assert_eq!(5u8 ^ 1u8, 4);
    /// assert_eq!(5u8 ^ 2u8, 7);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn bitxor(self, rhs: Rhs) -> Self::Output;
}

macro_rules! bitxor_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitXor for $t {
            type Output = $t;

            #[inline]
            fn bitxor(self, other: $t) -> $t { self ^ other }
        }

        forward_ref_binop! { impl BitXor, bitxor for $t, $t,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )*)
}

bitxor_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// 左移运算符 `<<`。注意,由于这个 trait 是为所有整数类型、且配以多种右操作数
/// 类型实现的,Rust 的类型检查器对 `_ << _` 做了特殊处理:它把整数运算的结果
/// 类型设为左操作数的类型。这意味着,尽管 `a << b` 和 `a.shl(b)` 从求值的角度
/// 看完全相同,但就类型推断而言它们是不同的。
///
/// # 示例
///
/// 一个把整数上的 `<<` 运算提升到 `usize` 包装类型上的 `Shl` 实现。
///
/// ```
/// use std::ops::Shl;
///
/// #[derive(PartialEq, Debug)]
/// struct Scalar(usize);
///
/// impl Shl<Scalar> for Scalar {
///     type Output = Self;
///
///     fn shl(self, Self(rhs): Self) -> Self::Output {
///         let Self(lhs) = self;
///         Self(lhs << rhs)
///     }
/// }
///
/// assert_eq!(Scalar(4) << Scalar(2), Scalar(16));
/// ```
///
/// 一个把向量向左旋转给定位数的 `Shl` 实现。
///
/// ```
/// use std::ops::Shl;
///
/// #[derive(PartialEq, Debug)]
/// struct SpinVector<T: Clone> {
///     vec: Vec<T>,
/// }
///
/// impl<T: Clone> Shl<usize> for SpinVector<T> {
///     type Output = Self;
///
///     fn shl(self, rhs: usize) -> Self::Output {
///         // 把向量旋转 `rhs` 个位置。
///         let (a, b) = self.vec.split_at(rhs);
///         let mut spun_vector = vec![];
///         spun_vector.extend_from_slice(b);
///         spun_vector.extend_from_slice(a);
///         Self { vec: spun_vector }
///     }
/// }
///
/// assert_eq!(SpinVector { vec: vec![0, 1, 2, 3, 4] } << 2,
///            SpinVector { vec: vec![2, 3, 4, 0, 1] });
/// ```
#[lang = "shl"]
#[doc(alias = "<<")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "no implementation for `{Self} << {Rhs}`",
    label = "no implementation for `{Self} << {Rhs}`"
)]
pub const trait Shl<Rhs = Self> {
    /// 应用 `<<` 运算符之后得到的结果类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// 执行 `<<` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(5u8 << 1, 10);
    /// assert_eq!(1u8 << 1, 2);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn shl(self, rhs: Rhs) -> Self::Output;
}

macro_rules! shl_impl {
    ($t:ty, $f:ty) => {
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Shl<$f> for $t {
            type Output = $t;

            #[inline]
            #[rustc_inherit_overflow_checks]
            fn shl(self, other: $f) -> $t {
                self << other
            }
        }

        forward_ref_binop! { impl Shl, shl for $t, $f,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    };
}

macro_rules! shl_impl_all {
    ($($t:ty)*) => ($(
        shl_impl! { $t, u8 }
        shl_impl! { $t, u16 }
        shl_impl! { $t, u32 }
        shl_impl! { $t, u64 }
        shl_impl! { $t, u128 }
        shl_impl! { $t, usize }

        shl_impl! { $t, i8 }
        shl_impl! { $t, i16 }
        shl_impl! { $t, i32 }
        shl_impl! { $t, i64 }
        shl_impl! { $t, i128 }
        shl_impl! { $t, isize }
    )*)
}

shl_impl_all! { u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize }

/// 右移运算符 `>>`。注意,由于这个 trait 是为所有整数类型、且配以多种右操作数
/// 类型实现的,Rust 的类型检查器对 `_ >> _` 做了特殊处理:它把整数运算的结果
/// 类型设为左操作数的类型。这意味着,尽管 `a >> b` 和 `a.shr(b)` 从求值的角度
/// 看完全相同,但就类型推断而言它们是不同的。
///
/// # 示例
///
/// 一个把整数上的 `>>` 运算提升到 `usize` 包装类型上的 `Shr` 实现。
///
/// ```
/// use std::ops::Shr;
///
/// #[derive(PartialEq, Debug)]
/// struct Scalar(usize);
///
/// impl Shr<Scalar> for Scalar {
///     type Output = Self;
///
///     fn shr(self, Self(rhs): Self) -> Self::Output {
///         let Self(lhs) = self;
///         Self(lhs >> rhs)
///     }
/// }
///
/// assert_eq!(Scalar(16) >> Scalar(2), Scalar(4));
/// ```
///
/// 一个把向量向右旋转给定位数的 `Shr` 实现。
///
/// ```
/// use std::ops::Shr;
///
/// #[derive(PartialEq, Debug)]
/// struct SpinVector<T: Clone> {
///     vec: Vec<T>,
/// }
///
/// impl<T: Clone> Shr<usize> for SpinVector<T> {
///     type Output = Self;
///
///     fn shr(self, rhs: usize) -> Self::Output {
///         // 把向量旋转 `rhs` 个位置。
///         let (a, b) = self.vec.split_at(self.vec.len() - rhs);
///         let mut spun_vector = vec![];
///         spun_vector.extend_from_slice(b);
///         spun_vector.extend_from_slice(a);
///         Self { vec: spun_vector }
///     }
/// }
///
/// assert_eq!(SpinVector { vec: vec![0, 1, 2, 3, 4] } >> 2,
///            SpinVector { vec: vec![3, 4, 0, 1, 2] });
/// ```
#[lang = "shr"]
#[doc(alias = ">>")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "no implementation for `{Self} >> {Rhs}`",
    label = "no implementation for `{Self} >> {Rhs}`"
)]
pub const trait Shr<Rhs = Self> {
    /// 应用 `>>` 运算符之后得到的结果类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// 执行 `>>` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(5u8 >> 1, 2);
    /// assert_eq!(2u8 >> 1, 1);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn shr(self, rhs: Rhs) -> Self::Output;
}

macro_rules! shr_impl {
    ($t:ty, $f:ty) => {
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Shr<$f> for $t {
            type Output = $t;

            #[inline]
            #[rustc_inherit_overflow_checks]
            fn shr(self, other: $f) -> $t {
                self >> other
            }
        }

        forward_ref_binop! { impl Shr, shr for $t, $f,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    };
}

macro_rules! shr_impl_all {
    ($($t:ty)*) => ($(
        shr_impl! { $t, u8 }
        shr_impl! { $t, u16 }
        shr_impl! { $t, u32 }
        shr_impl! { $t, u64 }
        shr_impl! { $t, u128 }
        shr_impl! { $t, usize }

        shr_impl! { $t, i8 }
        shr_impl! { $t, i16 }
        shr_impl! { $t, i32 }
        shr_impl! { $t, i64 }
        shr_impl! { $t, i128 }
        shr_impl! { $t, isize }
    )*)
}

shr_impl_all! { u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize }

/// 按位与赋值运算符 `&=`。
///
/// # 示例
///
/// 一个把 `&=` 运算符提升到 `bool` 包装类型上的 `BitAndAssign` 实现。
///
/// ```
/// use std::ops::BitAndAssign;
///
/// #[derive(Debug, PartialEq)]
/// struct Scalar(bool);
///
/// impl BitAndAssign for Scalar {
///     // rhs 是表达式 `a &= b` 的“右操作数”(right-hand side)
///     fn bitand_assign(&mut self, rhs: Self) {
///         *self = Self(self.0 & rhs.0)
///     }
/// }
///
/// let mut scalar = Scalar(true);
/// scalar &= Scalar(true);
/// assert_eq!(scalar, Scalar(true));
///
/// let mut scalar = Scalar(true);
/// scalar &= Scalar(false);
/// assert_eq!(scalar, Scalar(false));
///
/// let mut scalar = Scalar(false);
/// scalar &= Scalar(true);
/// assert_eq!(scalar, Scalar(false));
///
/// let mut scalar = Scalar(false);
/// scalar &= Scalar(false);
/// assert_eq!(scalar, Scalar(false));
/// ```
///
/// 这里为一个 `Vec<bool>` 的包装类型实现了 `BitAndAssign` trait。
///
/// ```
/// use std::ops::BitAndAssign;
///
/// #[derive(Debug, PartialEq)]
/// struct BooleanVector(Vec<bool>);
///
/// impl BitAndAssign for BooleanVector {
///     // `rhs` 是表达式 `a &= b` 的“右操作数”(right-hand side)。
///     fn bitand_assign(&mut self, rhs: Self) {
///         assert_eq!(self.0.len(), rhs.0.len());
///         *self = Self(
///             self.0
///                 .iter()
///                 .zip(rhs.0.iter())
///                 .map(|(x, y)| *x & *y)
///                 .collect()
///         );
///     }
/// }
///
/// let mut bv = BooleanVector(vec![true, true, false, false]);
/// bv &= BooleanVector(vec![true, false, true, false]);
/// let expected = BooleanVector(vec![true, false, false, false]);
/// assert_eq!(bv, expected);
/// ```
#[lang = "bitand_assign"]
#[doc(alias = "&=")]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "no implementation for `{Self} &= {Rhs}`",
    label = "no implementation for `{Self} &= {Rhs}`"
)]
pub const trait BitAndAssign<Rhs = Self> {
    /// 执行 `&=` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut x = true;
    /// x &= false;
    /// assert_eq!(x, false);
    ///
    /// let mut x = true;
    /// x &= true;
    /// assert_eq!(x, true);
    ///
    /// let mut x: u8 = 5;
    /// x &= 1;
    /// assert_eq!(x, 1);
    ///
    /// let mut x: u8 = 5;
    /// x &= 2;
    /// assert_eq!(x, 0);
    /// ```
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn bitand_assign(&mut self, rhs: Rhs);
}

macro_rules! bitand_assign_impl {
    ($($t:ty)+) => ($(
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitAndAssign for $t {
            #[inline]
            fn bitand_assign(&mut self, other: $t) { *self &= other }
        }

        forward_ref_op_assign! { impl BitAndAssign, bitand_assign for $t, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )+)
}

bitand_assign_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// 按位或赋值运算符 `|=`。
///
/// # 示例
///
/// ```
/// use std::ops::BitOrAssign;
///
/// #[derive(Debug, PartialEq)]
/// struct PersonalPreferences {
///     likes_cats: bool,
///     likes_dogs: bool,
/// }
///
/// impl BitOrAssign for PersonalPreferences {
///     fn bitor_assign(&mut self, rhs: Self) {
///         self.likes_cats |= rhs.likes_cats;
///         self.likes_dogs |= rhs.likes_dogs;
///     }
/// }
///
/// let mut prefs = PersonalPreferences { likes_cats: true, likes_dogs: false };
/// prefs |= PersonalPreferences { likes_cats: false, likes_dogs: true };
/// assert_eq!(prefs, PersonalPreferences { likes_cats: true, likes_dogs: true });
/// ```
#[lang = "bitor_assign"]
#[doc(alias = "|=")]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "no implementation for `{Self} |= {Rhs}`",
    label = "no implementation for `{Self} |= {Rhs}`"
)]
pub const trait BitOrAssign<Rhs = Self> {
    /// 执行 `|=` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut x = true;
    /// x |= false;
    /// assert_eq!(x, true);
    ///
    /// let mut x = false;
    /// x |= false;
    /// assert_eq!(x, false);
    ///
    /// let mut x: u8 = 5;
    /// x |= 1;
    /// assert_eq!(x, 5);
    ///
    /// let mut x: u8 = 5;
    /// x |= 2;
    /// assert_eq!(x, 7);
    /// ```
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn bitor_assign(&mut self, rhs: Rhs);
}

macro_rules! bitor_assign_impl {
    ($($t:ty)+) => ($(
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitOrAssign for $t {
            #[inline]
            fn bitor_assign(&mut self, other: $t) { *self |= other }
        }

        forward_ref_op_assign! { impl BitOrAssign, bitor_assign for $t, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )+)
}

bitor_assign_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// 按位异或赋值运算符 `^=`。
///
/// # 示例
///
/// ```
/// use std::ops::BitXorAssign;
///
/// #[derive(Debug, PartialEq)]
/// struct Personality {
///     has_soul: bool,
///     likes_knitting: bool,
/// }
///
/// impl BitXorAssign for Personality {
///     fn bitxor_assign(&mut self, rhs: Self) {
///         self.has_soul ^= rhs.has_soul;
///         self.likes_knitting ^= rhs.likes_knitting;
///     }
/// }
///
/// let mut personality = Personality { has_soul: false, likes_knitting: true };
/// personality ^= Personality { has_soul: true, likes_knitting: true };
/// assert_eq!(personality, Personality { has_soul: true, likes_knitting: false});
/// ```
#[lang = "bitxor_assign"]
#[doc(alias = "^=")]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "no implementation for `{Self} ^= {Rhs}`",
    label = "no implementation for `{Self} ^= {Rhs}`"
)]
pub const trait BitXorAssign<Rhs = Self> {
    /// 执行 `^=` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut x = true;
    /// x ^= false;
    /// assert_eq!(x, true);
    ///
    /// let mut x = true;
    /// x ^= true;
    /// assert_eq!(x, false);
    ///
    /// let mut x: u8 = 5;
    /// x ^= 1;
    /// assert_eq!(x, 4);
    ///
    /// let mut x: u8 = 5;
    /// x ^= 2;
    /// assert_eq!(x, 7);
    /// ```
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn bitxor_assign(&mut self, rhs: Rhs);
}

macro_rules! bitxor_assign_impl {
    ($($t:ty)+) => ($(
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitXorAssign for $t {
            #[inline]
            fn bitxor_assign(&mut self, other: $t) { *self ^= other }
        }

        forward_ref_op_assign! { impl BitXorAssign, bitxor_assign for $t, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )+)
}

bitxor_assign_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// 左移赋值运算符 `<<=`。
///
/// # 示例
///
/// 为一个 `usize` 的包装类型实现 `ShlAssign`。
///
/// ```
/// use std::ops::ShlAssign;
///
/// #[derive(Debug, PartialEq)]
/// struct Scalar(usize);
///
/// impl ShlAssign<usize> for Scalar {
///     fn shl_assign(&mut self, rhs: usize) {
///         self.0 <<= rhs;
///     }
/// }
///
/// let mut scalar = Scalar(4);
/// scalar <<= 2;
/// assert_eq!(scalar, Scalar(16));
/// ```
#[lang = "shl_assign"]
#[doc(alias = "<<=")]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "no implementation for `{Self} <<= {Rhs}`",
    label = "no implementation for `{Self} <<= {Rhs}`"
)]
pub const trait ShlAssign<Rhs = Self> {
    /// 执行 `<<=` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut x: u8 = 5;
    /// x <<= 1;
    /// assert_eq!(x, 10);
    ///
    /// let mut x: u8 = 1;
    /// x <<= 1;
    /// assert_eq!(x, 2);
    /// ```
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn shl_assign(&mut self, rhs: Rhs);
}

macro_rules! shl_assign_impl {
    ($t:ty, $f:ty) => {
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const ShlAssign<$f> for $t {
            #[inline]
            #[rustc_inherit_overflow_checks]
            fn shl_assign(&mut self, other: $f) {
                *self <<= other
            }
        }

        forward_ref_op_assign! { impl ShlAssign, shl_assign for $t, $f,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    };
}

macro_rules! shl_assign_impl_all {
    ($($t:ty)*) => ($(
        shl_assign_impl! { $t, u8 }
        shl_assign_impl! { $t, u16 }
        shl_assign_impl! { $t, u32 }
        shl_assign_impl! { $t, u64 }
        shl_assign_impl! { $t, u128 }
        shl_assign_impl! { $t, usize }

        shl_assign_impl! { $t, i8 }
        shl_assign_impl! { $t, i16 }
        shl_assign_impl! { $t, i32 }
        shl_assign_impl! { $t, i64 }
        shl_assign_impl! { $t, i128 }
        shl_assign_impl! { $t, isize }
    )*)
}

shl_assign_impl_all! { u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize }

/// 右移赋值运算符 `>>=`。
///
/// # 示例
///
/// 为一个 `usize` 的包装类型实现 `ShrAssign`。
///
/// ```
/// use std::ops::ShrAssign;
///
/// #[derive(Debug, PartialEq)]
/// struct Scalar(usize);
///
/// impl ShrAssign<usize> for Scalar {
///     fn shr_assign(&mut self, rhs: usize) {
///         self.0 >>= rhs;
///     }
/// }
///
/// let mut scalar = Scalar(16);
/// scalar >>= 2;
/// assert_eq!(scalar, Scalar(4));
/// ```
#[lang = "shr_assign"]
#[doc(alias = ">>=")]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "no implementation for `{Self} >>= {Rhs}`",
    label = "no implementation for `{Self} >>= {Rhs}`"
)]
pub const trait ShrAssign<Rhs = Self> {
    /// 执行 `>>=` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut x: u8 = 5;
    /// x >>= 1;
    /// assert_eq!(x, 2);
    ///
    /// let mut x: u8 = 2;
    /// x >>= 1;
    /// assert_eq!(x, 1);
    /// ```
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn shr_assign(&mut self, rhs: Rhs);
}

macro_rules! shr_assign_impl {
    ($t:ty, $f:ty) => {
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const ShrAssign<$f> for $t {
            #[inline]
            #[rustc_inherit_overflow_checks]
            fn shr_assign(&mut self, other: $f) {
                *self >>= other
            }
        }

        forward_ref_op_assign! { impl ShrAssign, shr_assign for $t, $f,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    };
}

macro_rules! shr_assign_impl_all {
    ($($t:ty)*) => ($(
        shr_assign_impl! { $t, u8 }
        shr_assign_impl! { $t, u16 }
        shr_assign_impl! { $t, u32 }
        shr_assign_impl! { $t, u64 }
        shr_assign_impl! { $t, u128 }
        shr_assign_impl! { $t, usize }

        shr_assign_impl! { $t, i8 }
        shr_assign_impl! { $t, i16 }
        shr_assign_impl! { $t, i32 }
        shr_assign_impl! { $t, i64 }
        shr_assign_impl! { $t, i128 }
        shr_assign_impl! { $t, isize }
    )*)
}

shr_assign_impl_all! { u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize }
