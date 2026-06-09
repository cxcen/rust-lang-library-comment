/// 加法运算符 `+`。
///
/// 注意 `Rhs` 默认是 `Self`,但这并非强制。例如 [`std::time::SystemTime`] 实现了
/// `Add<Duration>`,从而允许形如 `SystemTime = SystemTime + Duration` 的运算。
///
/// [`std::time::SystemTime`]: ../../std/time/struct.SystemTime.html
///
/// # 示例
///
/// ## 可做 `Add`(加法)的点
///
/// ```
/// use std::ops::Add;
///
/// #[derive(Debug, Copy, Clone, PartialEq)]
/// struct Point {
///     x: i32,
///     y: i32,
/// }
///
/// impl Add for Point {
///     type Output = Self;
///
///     fn add(self, other: Self) -> Self {
///         Self {
///             x: self.x + other.x,
///             y: self.y + other.y,
///         }
///     }
/// }
///
/// assert_eq!(Point { x: 1, y: 0 } + Point { x: 2, y: 3 },
///            Point { x: 3, y: 3 });
/// ```
///
/// ## 用泛型实现 `Add`
///
/// 下面是同一个 `Point` 结构体使用泛型实现 `Add` trait 的示例。
///
/// ```
/// use std::ops::Add;
///
/// #[derive(Debug, Copy, Clone, PartialEq)]
/// struct Point<T> {
///     x: T,
///     y: T,
/// }
///
/// // 注意该实现使用了关联类型 `Output`。
/// impl<T: Add<Output = T>> Add for Point<T> {
///     type Output = Self;
///
///     fn add(self, other: Self) -> Self::Output {
///         Self {
///             x: self.x + other.x,
///             y: self.y + other.y,
///         }
///     }
/// }
///
/// assert_eq!(Point { x: 1, y: 0 } + Point { x: 2, y: 3 },
///            Point { x: 3, y: 3 });
/// ```
#[lang = "add"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[rustc_on_unimplemented(
    on(all(Self = "{integer}", Rhs = "{float}"), message = "cannot add a float to an integer",),
    on(all(Self = "{float}", Rhs = "{integer}"), message = "cannot add an integer to a float",),
    message = "cannot add `{Rhs}` to `{Self}`",
    label = "no implementation for `{Self} + {Rhs}`",
    append_const_msg
)]
#[doc(alias = "+")]
pub const trait Add<Rhs = Self> {
    /// 应用 `+` 运算符之后得到的结果类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// 执行 `+` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(12 + 1, 13);
    /// ```
    #[must_use = "this returns the result of the operation, without modifying the original"]
    #[rustc_diagnostic_item = "add"]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn add(self, rhs: Rhs) -> Self::Output;
}

macro_rules! add_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Add for $t {
            type Output = $t;

            #[inline]
            #[track_caller]
            #[rustc_inherit_overflow_checks]
            fn add(self, other: $t) -> $t { self + other }
        }

        forward_ref_binop! { impl Add, add for $t, $t,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )*)
}

add_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 f16 f32 f64 f128 }

/// 减法运算符 `-`。
///
/// 注意 `Rhs` 默认是 `Self`,但这并非强制。例如 [`std::time::SystemTime`] 实现了
/// `Sub<Duration>`,从而允许形如 `SystemTime = SystemTime - Duration` 的运算。
///
/// [`std::time::SystemTime`]: ../../std/time/struct.SystemTime.html
///
/// # 示例
///
/// ## 可做 `Sub`(减法)的点
///
/// ```
/// use std::ops::Sub;
///
/// #[derive(Debug, Copy, Clone, PartialEq)]
/// struct Point {
///     x: i32,
///     y: i32,
/// }
///
/// impl Sub for Point {
///     type Output = Self;
///
///     fn sub(self, other: Self) -> Self::Output {
///         Self {
///             x: self.x - other.x,
///             y: self.y - other.y,
///         }
///     }
/// }
///
/// assert_eq!(Point { x: 3, y: 3 } - Point { x: 2, y: 3 },
///            Point { x: 1, y: 0 });
/// ```
///
/// ## 用泛型实现 `Sub`
///
/// 下面是同一个 `Point` 结构体使用泛型实现 `Sub` trait 的示例。
///
/// ```
/// use std::ops::Sub;
///
/// #[derive(Debug, PartialEq)]
/// struct Point<T> {
///     x: T,
///     y: T,
/// }
///
/// // 注意该实现使用了关联类型 `Output`。
/// impl<T: Sub<Output = T>> Sub for Point<T> {
///     type Output = Self;
///
///     fn sub(self, other: Self) -> Self::Output {
///         Point {
///             x: self.x - other.x,
///             y: self.y - other.y,
///         }
///     }
/// }
///
/// assert_eq!(Point { x: 2, y: 3 } - Point { x: 1, y: 0 },
///            Point { x: 1, y: 3 });
/// ```
#[lang = "sub"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[rustc_on_unimplemented(
    message = "cannot subtract `{Rhs}` from `{Self}`",
    label = "no implementation for `{Self} - {Rhs}`",
    append_const_msg
)]
#[doc(alias = "-")]
pub const trait Sub<Rhs = Self> {
    /// 应用 `-` 运算符之后得到的结果类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// 执行 `-` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(12 - 1, 11);
    /// ```
    #[must_use = "this returns the result of the operation, without modifying the original"]
    #[rustc_diagnostic_item = "sub"]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn sub(self, rhs: Rhs) -> Self::Output;
}

macro_rules! sub_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Sub for $t {
            type Output = $t;

            #[inline]
            #[track_caller]
            #[rustc_inherit_overflow_checks]
            fn sub(self, other: $t) -> $t { self - other }
        }

        forward_ref_binop! { impl Sub, sub for $t, $t,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )*)
}

sub_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 f16 f32 f64 f128 }

/// 乘法运算符 `*`。
///
/// 注意 `Rhs` 默认是 `Self`,但这并非强制。
///
/// # 示例
///
/// ## 可做 `Mul`(乘法)的有理数
///
/// ```
/// use std::ops::Mul;
///
/// // 根据算术基本定理,化为最简形式的有理数是唯一的。因此,只要让 `Rational`
/// // 始终保持约分后的形式,我们就能 derive `Eq` 和 `PartialEq`。
/// #[derive(Debug, Eq, PartialEq)]
/// struct Rational {
///     numerator: usize,
///     denominator: usize,
/// }
///
/// impl Rational {
///     fn new(numerator: usize, denominator: usize) -> Self {
///         if denominator == 0 {
///             panic!("Zero is an invalid denominator!");
///         }
///
///         // 通过除以最大公约数来约分到最简形式。
///         let gcd = gcd(numerator, denominator);
///         Self {
///             numerator: numerator / gcd,
///             denominator: denominator / gcd,
///         }
///     }
/// }
///
/// impl Mul for Rational {
///     // 有理数的乘法是一个封闭运算。
///     type Output = Self;
///
///     fn mul(self, rhs: Self) -> Self {
///         let numerator = self.numerator * rhs.numerator;
///         let denominator = self.denominator * rhs.denominator;
///         Self::new(numerator, denominator)
///     }
/// }
///
/// // 欧几里得用于求最大公约数的两千年前的算法。
/// fn gcd(x: usize, y: usize) -> usize {
///     let mut x = x;
///     let mut y = y;
///     while y != 0 {
///         let t = y;
///         y = x % y;
///         x = t;
///     }
///     x
/// }
///
/// assert_eq!(Rational::new(1, 2), Rational::new(2, 4));
/// assert_eq!(Rational::new(2, 3) * Rational::new(3, 4),
///            Rational::new(1, 2));
/// ```
///
/// ## 像线性代数那样用标量去乘向量
///
/// ```
/// use std::ops::Mul;
///
/// struct Scalar { value: usize }
///
/// #[derive(Debug, PartialEq)]
/// struct Vector { value: Vec<usize> }
///
/// impl Mul<Scalar> for Vector {
///     type Output = Self;
///
///     fn mul(self, rhs: Scalar) -> Self::Output {
///         Self { value: self.value.iter().map(|v| v * rhs.value).collect() }
///     }
/// }
///
/// let vector = Vector { value: vec![2, 4, 6] };
/// let scalar = Scalar { value: 3 };
/// assert_eq!(vector * scalar, Vector { value: vec![6, 12, 18] });
/// ```
#[lang = "mul"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "cannot multiply `{Self}` by `{Rhs}`",
    label = "no implementation for `{Self} * {Rhs}`"
)]
#[doc(alias = "*")]
pub const trait Mul<Rhs = Self> {
    /// 应用 `*` 运算符之后得到的结果类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// 执行 `*` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(12 * 2, 24);
    /// ```
    #[must_use = "this returns the result of the operation, without modifying the original"]
    #[rustc_diagnostic_item = "mul"]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn mul(self, rhs: Rhs) -> Self::Output;
}

macro_rules! mul_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Mul for $t {
            type Output = $t;

            #[inline]
            #[track_caller]
            #[rustc_inherit_overflow_checks]
            fn mul(self, other: $t) -> $t { self * other }
        }

        forward_ref_binop! { impl Mul, mul for $t, $t,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )*)
}

mul_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 f16 f32 f64 f128 }

/// 除法运算符 `/`。
///
/// 注意 `Rhs` 默认是 `Self`,但这并非强制。
///
/// # 示例
///
/// ## 可做 `Div`(除法)的有理数
///
/// ```
/// use std::ops::Div;
///
/// // 根据算术基本定理,化为最简形式的有理数是唯一的。因此,只要让 `Rational`
/// // 始终保持约分后的形式,我们就能 derive `Eq` 和 `PartialEq`。
/// #[derive(Debug, Eq, PartialEq)]
/// struct Rational {
///     numerator: usize,
///     denominator: usize,
/// }
///
/// impl Rational {
///     fn new(numerator: usize, denominator: usize) -> Self {
///         if denominator == 0 {
///             panic!("Zero is an invalid denominator!");
///         }
///
///         // 通过除以最大公约数来约分到最简形式。
///         let gcd = gcd(numerator, denominator);
///         Self {
///             numerator: numerator / gcd,
///             denominator: denominator / gcd,
///         }
///     }
/// }
///
/// impl Div for Rational {
///     // 有理数的除法是一个封闭运算。
///     type Output = Self;
///
///     fn div(self, rhs: Self) -> Self::Output {
///         if rhs.numerator == 0 {
///             panic!("Cannot divide by zero-valued `Rational`!");
///         }
///
///         let numerator = self.numerator * rhs.denominator;
///         let denominator = self.denominator * rhs.numerator;
///         Self::new(numerator, denominator)
///     }
/// }
///
/// // 欧几里得用于求最大公约数的两千年前的算法。
/// fn gcd(x: usize, y: usize) -> usize {
///     let mut x = x;
///     let mut y = y;
///     while y != 0 {
///         let t = y;
///         y = x % y;
///         x = t;
///     }
///     x
/// }
///
/// assert_eq!(Rational::new(1, 2), Rational::new(2, 4));
/// assert_eq!(Rational::new(1, 2) / Rational::new(3, 4),
///            Rational::new(2, 3));
/// ```
///
/// ## 像线性代数那样用标量去除向量
///
/// ```
/// use std::ops::Div;
///
/// struct Scalar { value: f32 }
///
/// #[derive(Debug, PartialEq)]
/// struct Vector { value: Vec<f32> }
///
/// impl Div<Scalar> for Vector {
///     type Output = Self;
///
///     fn div(self, rhs: Scalar) -> Self::Output {
///         Self { value: self.value.iter().map(|v| v / rhs.value).collect() }
///     }
/// }
///
/// let scalar = Scalar { value: 2f32 };
/// let vector = Vector { value: vec![2f32, 4f32, 6f32] };
/// assert_eq!(vector / scalar, Vector { value: vec![1f32, 2f32, 3f32] });
/// ```
#[lang = "div"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "cannot divide `{Self}` by `{Rhs}`",
    label = "no implementation for `{Self} / {Rhs}`"
)]
#[doc(alias = "/")]
pub const trait Div<Rhs = Self> {
    /// 应用 `/` 运算符之后得到的结果类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// 执行 `/` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(12 / 2, 6);
    /// ```
    #[must_use = "this returns the result of the operation, without modifying the original"]
    #[rustc_diagnostic_item = "div"]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn div(self, rhs: Rhs) -> Self::Output;
}

macro_rules! div_impl_integer {
    ($(($($t:ty)*) => $panic:expr),*) => ($($(
        /// 此运算向零取整(round towards zero),即截断精确结果中所有的小数部分。
        ///
        /// # Panics
        ///
        #[doc = $panic]
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Div for $t {
            type Output = $t;

            #[inline]
            #[track_caller]
            fn div(self, other: $t) -> $t { self / other }
        }

        forward_ref_binop! { impl Div, div for $t, $t,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )*)*)
}

div_impl_integer! {
    (usize u8 u16 u32 u64 u128) => "若 `other == 0`,此运算会 panic。",
    (isize i8 i16 i32 i64 i128) => "若 `other == 0`,或者除法结果发生溢出,此运算会 panic。"
}

macro_rules! div_impl_float {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Div for $t {
            type Output = $t;

            #[inline]
            fn div(self, other: $t) -> $t { self / other }
        }

        forward_ref_binop! { impl Div, div for $t, $t,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )*)
}

div_impl_float! { f16 f32 f64 f128 }

/// 求余运算符 `%`。
///
/// 注意 `Rhs` 默认是 `Self`,但这并非强制。
///
/// # 示例
///
/// 本示例为一个 `SplitSlice` 对象实现了 `Rem`。实现 `Rem` 之后,就能用 `%`
/// 运算符求出:在把切片切成若干个给定长度的等长子切片之后,剩余的元素会是什么。
///
/// ```
/// use std::ops::Rem;
///
/// #[derive(PartialEq, Debug)]
/// struct SplitSlice<'a, T> {
///     slice: &'a [T],
/// }
///
/// impl<'a, T> Rem<usize> for SplitSlice<'a, T> {
///     type Output = Self;
///
///     fn rem(self, modulus: usize) -> Self::Output {
///         let len = self.slice.len();
///         let rem = len % modulus;
///         let start = len - rem;
///         Self {slice: &self.slice[start..]}
///     }
/// }
///
/// // 如果我们把 &[0, 1, 2, 3, 4, 5, 6, 7] 切成大小为 3 的子切片,
/// // 余下的部分会是 &[6, 7]。
/// assert_eq!(SplitSlice { slice: &[0, 1, 2, 3, 4, 5, 6, 7] } % 3,
///            SplitSlice { slice: &[6, 7] });
/// ```
#[lang = "rem"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "cannot calculate the remainder of `{Self}` divided by `{Rhs}`",
    label = "no implementation for `{Self} % {Rhs}`"
)]
#[doc(alias = "%")]
pub const trait Rem<Rhs = Self> {
    /// 应用 `%` 运算符之后得到的结果类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// 执行 `%` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(12 % 10, 2);
    /// ```
    #[must_use = "this returns the result of the operation, without modifying the original"]
    #[rustc_diagnostic_item = "rem"]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn rem(self, rhs: Rhs) -> Self::Output;
}

macro_rules! rem_impl_integer {
    ($(($($t:ty)*) => $panic:expr),*) => ($($(
        /// 此运算满足 `n % d == n - (n / d) * d`。其结果与左操作数符号相同。
        ///
        /// # Panics
        ///
        #[doc = $panic]
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Rem for $t {
            type Output = $t;

            #[inline]
            #[track_caller]
            fn rem(self, other: $t) -> $t { self % other }
        }

        forward_ref_binop! { impl Rem, rem for $t, $t,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )*)*)
}

rem_impl_integer! {
    (usize u8 u16 u32 u64 u128) => "若 `other == 0`,此运算会 panic。",
    (isize i8 i16 i32 i64 i128) => "若 `other == 0`,或者 `self / other` 发生溢出,此运算会 panic。"
}

macro_rules! rem_impl_float {
    ($($t:ty)*) => ($(

        /// 两个浮点数相除得到的余数。
        ///
        /// 该余数与被除数符号相同,按 `x - (x / y).trunc() * y` 计算。
        ///
        /// # 示例
        /// ```
        /// let x: f32 = 50.50;
        /// let y: f32 = 8.125;
        /// let remainder = x - (x / y).trunc() * y;
        ///
        /// // 两种运算的结果都是 1.75
        /// assert_eq!(x % y, remainder);
        /// ```
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Rem for $t {
            type Output = $t;

            #[inline]
            fn rem(self, other: $t) -> $t { self % other }
        }

        forward_ref_binop! { impl Rem, rem for $t, $t,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )*)
}

rem_impl_float! { f16 f32 f64 f128 }

/// 一元取负运算符 `-`。
///
/// # 示例
///
/// 为 `Sign` 实现 `Neg`,从而允许用 `-` 对其取负。
///
/// ```
/// use std::ops::Neg;
///
/// #[derive(Debug, PartialEq)]
/// enum Sign {
///     Negative,
///     Zero,
///     Positive,
/// }
///
/// impl Neg for Sign {
///     type Output = Self;
///
///     fn neg(self) -> Self::Output {
///         match self {
///             Sign::Negative => Sign::Positive,
///             Sign::Zero => Sign::Zero,
///             Sign::Positive => Sign::Negative,
///         }
///     }
/// }
///
/// // 正的相反数是负的。
/// assert_eq!(-Sign::Positive, Sign::Negative);
/// // 负负得正。
/// assert_eq!(-Sign::Negative, Sign::Positive);
/// // 零的相反数还是它自己。
/// assert_eq!(-Sign::Zero, Sign::Zero);
/// ```
#[lang = "neg"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[doc(alias = "-")]
pub const trait Neg {
    /// 应用 `-` 运算符之后得到的结果类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// 执行一元 `-` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// let x: i32 = 12;
    /// assert_eq!(-x, -12);
    /// ```
    #[must_use = "this returns the result of the operation, without modifying the original"]
    #[rustc_diagnostic_item = "neg"]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn neg(self) -> Self::Output;
}

macro_rules! neg_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Neg for $t {
            type Output = $t;

            #[inline]
            #[rustc_inherit_overflow_checks]
            fn neg(self) -> $t { -self }
        }

        forward_ref_unop! { impl Neg, neg for $t,
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )*)
}

neg_impl! { isize i8 i16 i32 i64 i128 f16 f32 f64 f128 }

/// 加法赋值运算符 `+=`。
///
/// # 示例
///
/// 本示例创建了一个实现了 `AddAssign` trait 的 `Point` 结构体,然后演示对一个
/// 可变的 `Point` 做加法赋值。
///
/// ```
/// use std::ops::AddAssign;
///
/// #[derive(Debug, Copy, Clone, PartialEq)]
/// struct Point {
///     x: i32,
///     y: i32,
/// }
///
/// impl AddAssign for Point {
///     fn add_assign(&mut self, other: Self) {
///         *self = Self {
///             x: self.x + other.x,
///             y: self.y + other.y,
///         };
///     }
/// }
///
/// let mut point = Point { x: 1, y: 0 };
/// point += Point { x: 2, y: 3 };
/// assert_eq!(point, Point { x: 3, y: 3 });
/// ```
#[lang = "add_assign"]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "cannot add-assign `{Rhs}` to `{Self}`",
    label = "no implementation for `{Self} += {Rhs}`"
)]
#[doc(alias = "+")]
#[doc(alias = "+=")]
pub const trait AddAssign<Rhs = Self> {
    /// 执行 `+=` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut x: u32 = 12;
    /// x += 1;
    /// assert_eq!(x, 13);
    /// ```
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn add_assign(&mut self, rhs: Rhs);
}

macro_rules! add_assign_impl {
    ($($t:ty)+) => ($(
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const AddAssign for $t {
            #[inline]
            #[track_caller]
            #[rustc_inherit_overflow_checks]
            fn add_assign(&mut self, other: $t) { *self += other }
        }

        forward_ref_op_assign! { impl AddAssign, add_assign for $t, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )+)
}

add_assign_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 f16 f32 f64 f128 }

/// 减法赋值运算符 `-=`。
///
/// # 示例
///
/// 本示例创建了一个实现了 `SubAssign` trait 的 `Point` 结构体,然后演示对一个
/// 可变的 `Point` 做减法赋值。
///
/// ```
/// use std::ops::SubAssign;
///
/// #[derive(Debug, Copy, Clone, PartialEq)]
/// struct Point {
///     x: i32,
///     y: i32,
/// }
///
/// impl SubAssign for Point {
///     fn sub_assign(&mut self, other: Self) {
///         *self = Self {
///             x: self.x - other.x,
///             y: self.y - other.y,
///         };
///     }
/// }
///
/// let mut point = Point { x: 3, y: 3 };
/// point -= Point { x: 2, y: 3 };
/// assert_eq!(point, Point {x: 1, y: 0});
/// ```
#[lang = "sub_assign"]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "cannot subtract-assign `{Rhs}` from `{Self}`",
    label = "no implementation for `{Self} -= {Rhs}`"
)]
#[doc(alias = "-")]
#[doc(alias = "-=")]
pub const trait SubAssign<Rhs = Self> {
    /// 执行 `-=` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut x: u32 = 12;
    /// x -= 1;
    /// assert_eq!(x, 11);
    /// ```
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn sub_assign(&mut self, rhs: Rhs);
}

macro_rules! sub_assign_impl {
    ($($t:ty)+) => ($(
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const SubAssign for $t {
            #[inline]
            #[track_caller]
            #[rustc_inherit_overflow_checks]
            fn sub_assign(&mut self, other: $t) { *self -= other }
        }

        forward_ref_op_assign! { impl SubAssign, sub_assign for $t, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )+)
}

sub_assign_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 f16 f32 f64 f128 }

/// 乘法赋值运算符 `*=`。
///
/// # 示例
///
/// ```
/// use std::ops::MulAssign;
///
/// #[derive(Debug, PartialEq)]
/// struct Frequency { hertz: f64 }
///
/// impl MulAssign<f64> for Frequency {
///     fn mul_assign(&mut self, rhs: f64) {
///         self.hertz *= rhs;
///     }
/// }
///
/// let mut frequency = Frequency { hertz: 50.0 };
/// frequency *= 4.0;
/// assert_eq!(Frequency { hertz: 200.0 }, frequency);
/// ```
#[lang = "mul_assign"]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "cannot multiply-assign `{Self}` by `{Rhs}`",
    label = "no implementation for `{Self} *= {Rhs}`"
)]
#[doc(alias = "*")]
#[doc(alias = "*=")]
pub const trait MulAssign<Rhs = Self> {
    /// 执行 `*=` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut x: u32 = 12;
    /// x *= 2;
    /// assert_eq!(x, 24);
    /// ```
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn mul_assign(&mut self, rhs: Rhs);
}

macro_rules! mul_assign_impl {
    ($($t:ty)+) => ($(
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const MulAssign for $t {
            #[inline]
            #[track_caller]
            #[rustc_inherit_overflow_checks]
            fn mul_assign(&mut self, other: $t) { *self *= other }
        }

        forward_ref_op_assign! { impl MulAssign, mul_assign for $t, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )+)
}

mul_assign_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 f16 f32 f64 f128 }

/// 除法赋值运算符 `/=`。
///
/// # 示例
///
/// ```
/// use std::ops::DivAssign;
///
/// #[derive(Debug, PartialEq)]
/// struct Frequency { hertz: f64 }
///
/// impl DivAssign<f64> for Frequency {
///     fn div_assign(&mut self, rhs: f64) {
///         self.hertz /= rhs;
///     }
/// }
///
/// let mut frequency = Frequency { hertz: 200.0 };
/// frequency /= 4.0;
/// assert_eq!(Frequency { hertz: 50.0 }, frequency);
/// ```
#[lang = "div_assign"]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "cannot divide-assign `{Self}` by `{Rhs}`",
    label = "no implementation for `{Self} /= {Rhs}`"
)]
#[doc(alias = "/")]
#[doc(alias = "/=")]
pub const trait DivAssign<Rhs = Self> {
    /// 执行 `/=` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut x: u32 = 12;
    /// x /= 2;
    /// assert_eq!(x, 6);
    /// ```
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn div_assign(&mut self, rhs: Rhs);
}

macro_rules! div_assign_impl {
    ($($t:ty)+) => ($(
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const DivAssign for $t {
            #[inline]
            #[track_caller]
            fn div_assign(&mut self, other: $t) { *self /= other }
        }

        forward_ref_op_assign! { impl DivAssign, div_assign for $t, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )+)
}

div_assign_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 f16 f32 f64 f128 }

/// 求余赋值运算符 `%=`。
///
/// # 示例
///
/// ```
/// use std::ops::RemAssign;
///
/// struct CookieJar { cookies: u32 }
///
/// impl RemAssign<u32> for CookieJar {
///     fn rem_assign(&mut self, piles: u32) {
///         self.cookies %= piles;
///     }
/// }
///
/// let mut jar = CookieJar { cookies: 31 };
/// let piles = 4;
///
/// println!("Splitting up {} cookies into {} even piles!", jar.cookies, piles);
///
/// jar %= piles;
///
/// println!("{} cookies remain in the cookie jar!", jar.cookies);
/// ```
#[lang = "rem_assign"]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
#[diagnostic::on_unimplemented(
    message = "cannot calculate and assign the remainder of `{Self}` divided by `{Rhs}`",
    label = "no implementation for `{Self} %= {Rhs}`"
)]
#[doc(alias = "%")]
#[doc(alias = "%=")]
pub const trait RemAssign<Rhs = Self> {
    /// 执行 `%=` 操作。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut x: u32 = 12;
    /// x %= 10;
    /// assert_eq!(x, 2);
    /// ```
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn rem_assign(&mut self, rhs: Rhs);
}

macro_rules! rem_assign_impl {
    ($($t:ty)+) => ($(
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const RemAssign for $t {
            #[inline]
            #[track_caller]
            fn rem_assign(&mut self, other: $t) { *self %= other }
        }

        forward_ref_op_assign! { impl RemAssign, rem_assign for $t, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )+)
}

rem_assign_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 f16 f32 f64 f128 }
