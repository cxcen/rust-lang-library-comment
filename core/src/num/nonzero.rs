//! 已知不等于零的整数类型定义。
//!
//! `NonZero<T>` 把“值不为零”编码进类型。这个不变量既服务于 API 语义，也服务于布局优化：
//! 零位模式不能作为 `NonZero` 的有效值出现，因此编译器可以把零当作 niche 存放枚举判别
//! 信息，让 `Option<NonZero<T>>` 与 `T` 保持相同大小。

use super::{IntErrorKind, ParseIntError};
use crate::clone::{TrivialClone, UseCloned};
use crate::cmp::Ordering;
use crate::hash::{Hash, Hasher};
use crate::marker::{Destruct, Freeze, StructuralPartialEq};
use crate::ops::{BitOr, BitOrAssign, Div, DivAssign, Neg, Rem, RemAssign};
use crate::panic::{RefUnwindSafe, UnwindSafe};
use crate::str::FromStr;
use crate::{fmt, intrinsics, ptr, ub_checks};

/// 可为零的原始类型所实现的标记 trait。
///
/// 这是 <code>[NonZero]\<T></code> 的实现细节，随时可能消失或被替换。
///
/// # 安全性(Safety）
///
/// 实现该 trait 的类型必须是“零位模式有效”的原始类型。
///
/// 关联类型 `Self::NonZeroInner` 必须与 `Self` 具有相同大小和对齐，但通过 niche 和位有效性
/// 排除零，使下列 `transmute` 是 sound 的：
///
/// - 从 `Self::NonZeroInner` 到 `Option<Self::NonZeroInner>`
/// - 从 `Option<Self::NonZeroInner>` 到 `Self`
///
/// 因而 `Self::NonZeroInner` 到 `Self` 的转换也必须 sound。若实现者错误声明这些布局关系，
/// 编译器会基于不存在的 niche 优化枚举布局，导致后续读取产生 UB。
#[unstable(
    feature = "nonzero_internals",
    reason = "implementation detail which may disappear or be replaced at any time",
    issue = "none"
)]
pub unsafe trait ZeroablePrimitive: Sized + Copy + private::Sealed {
    #[doc(hidden)]
    type NonZeroInner: Sized + Copy;
}

macro_rules! impl_zeroable_primitive {
    ($($NonZeroInner:ident ( $primitive:ty )),+ $(,)?) => {
        mod private {
            #[unstable(
                feature = "nonzero_internals",
                reason = "implementation detail which may disappear or be replaced at any time",
                issue = "none"
            )]
            pub trait Sealed {}
        }

        $(
            #[unstable(
                feature = "nonzero_internals",
                reason = "implementation detail which may disappear or be replaced at any time",
                issue = "none"
            )]
            impl private::Sealed for $primitive {}

            #[unstable(
                feature = "nonzero_internals",
                reason = "implementation detail which may disappear or be replaced at any time",
                issue = "none"
            )]
            unsafe impl ZeroablePrimitive for $primitive {
                type NonZeroInner = super::niche_types::$NonZeroInner;
            }
        )+
    };
}

impl_zeroable_primitive!(
    NonZeroU8Inner(u8),
    NonZeroU16Inner(u16),
    NonZeroU32Inner(u32),
    NonZeroU64Inner(u64),
    NonZeroU128Inner(u128),
    NonZeroUsizeInner(usize),
    NonZeroI8Inner(i8),
    NonZeroI16Inner(i16),
    NonZeroI32Inner(i32),
    NonZeroI64Inner(i64),
    NonZeroI128Inner(i128),
    NonZeroIsizeInner(isize),
    NonZeroCharInner(char),
);

/// 一个已知不等于零的值。
///
/// 该类型启用一些内存布局优化。例如，`Option<NonZero<u32>>` 与 `u32` 大小相同：
///
/// ```
/// use core::{num::NonZero};
///
/// assert_eq!(size_of::<Option<NonZero<u32>>>(), size_of::<u32>());
/// ```
///
/// # 布局(Layout)
///
/// `NonZero<T>` 保证与 `T` 具有相同布局和位有效性，唯一例外是全零位模式无效。
/// `Option<NonZero<T>>` 保证与 `T` 兼容，包括 FFI 场景。
///
/// 得益于 [null pointer optimization]，`NonZero<T>` 和 `Option<NonZero<T>>` 保证具有相同
/// 大小和对齐：
///
/// ```
/// use std::num::NonZero;
///
/// assert_eq!(size_of::<NonZero<u32>>(), size_of::<Option<NonZero<u32>>>());
/// assert_eq!(align_of::<NonZero<u32>>(), align_of::<Option<NonZero<u32>>>());
/// ```
///
/// [null pointer optimization]: crate::option#representation
///
/// # 泛型用法说明
///
/// `NonZero<T>` 只能用于部分标准库原始类型（例如 `u8`、`i32` 等）。类型参数 `T` 必须实现
/// 内部 trait [`ZeroablePrimitive`]；该 trait 目前永久 unstable，用户不能自行实现。
/// 因此不能把 `NonZero<T>` 用在自定义类型上，也不能为所有 `NonZero<T>` 泛型实现 trait，
/// 只能为具体类型实现。
#[stable(feature = "generic_nonzero", since = "1.79.0")]
#[repr(transparent)]
#[rustc_nonnull_optimization_guaranteed]
#[rustc_diagnostic_item = "NonZero"]
pub struct NonZero<T: ZeroablePrimitive>(T::NonZeroInner);

macro_rules! impl_nonzero_fmt {
    ($(#[$Attribute:meta] $Trait:ident)*) => {
        $(
            #[$Attribute]
            impl<T> fmt::$Trait for NonZero<T>
            where
                T: ZeroablePrimitive + fmt::$Trait,
            {
                #[inline]
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.get().fmt(f)
                }
            }
        )*
    };
}

impl_nonzero_fmt! {
    #[stable(feature = "nonzero", since = "1.28.0")]
    Debug
    #[stable(feature = "nonzero", since = "1.28.0")]
    Display
    #[stable(feature = "nonzero", since = "1.28.0")]
    Binary
    #[stable(feature = "nonzero", since = "1.28.0")]
    Octal
    #[stable(feature = "nonzero", since = "1.28.0")]
    LowerHex
    #[stable(feature = "nonzero", since = "1.28.0")]
    UpperHex
    #[stable(feature = "nonzero_fmt_exp", since = "1.84.0")]
    LowerExp
    #[stable(feature = "nonzero_fmt_exp", since = "1.84.0")]
    UpperExp
}

macro_rules! impl_nonzero_auto_trait {
    (unsafe $Trait:ident) => {
        #[stable(feature = "nonzero", since = "1.28.0")]
        unsafe impl<T> $Trait for NonZero<T> where T: ZeroablePrimitive + $Trait {}
    };
    ($Trait:ident) => {
        #[stable(feature = "nonzero", since = "1.28.0")]
        impl<T> $Trait for NonZero<T> where T: ZeroablePrimitive + $Trait {}
    };
}

// 基于 `T` 手写 auto-trait 实现，避免文档暴露 `ZeroablePrimitive::NonZeroInner` 这个实现细节。
impl_nonzero_auto_trait!(unsafe Freeze);
impl_nonzero_auto_trait!(RefUnwindSafe);
impl_nonzero_auto_trait!(unsafe Send);
impl_nonzero_auto_trait!(unsafe Sync);
impl_nonzero_auto_trait!(Unpin);
impl_nonzero_auto_trait!(UnwindSafe);

#[stable(feature = "nonzero", since = "1.28.0")]
impl<T> Clone for NonZero<T>
where
    T: ZeroablePrimitive,
{
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

#[unstable(feature = "ergonomic_clones", issue = "132290")]
impl<T> UseCloned for NonZero<T> where T: ZeroablePrimitive {}

#[stable(feature = "nonzero", since = "1.28.0")]
impl<T> Copy for NonZero<T> where T: ZeroablePrimitive {}

#[doc(hidden)]
#[unstable(feature = "trivial_clone", issue = "none")]
unsafe impl<T> TrivialClone for NonZero<T> where T: ZeroablePrimitive {}

#[stable(feature = "nonzero", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<T> const PartialEq for NonZero<T>
where
    T: ZeroablePrimitive + [const] PartialEq,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }

    #[inline]
    fn ne(&self, other: &Self) -> bool {
        self.get() != other.get()
    }
}

#[unstable(feature = "structural_match", issue = "31434")]
impl<T> StructuralPartialEq for NonZero<T> where T: ZeroablePrimitive + StructuralPartialEq {}

#[stable(feature = "nonzero", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<T> const Eq for NonZero<T> where T: ZeroablePrimitive + [const] Eq {}

#[stable(feature = "nonzero", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<T> const PartialOrd for NonZero<T>
where
    T: ZeroablePrimitive + [const] PartialOrd,
{
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.get().partial_cmp(&other.get())
    }

    #[inline]
    fn lt(&self, other: &Self) -> bool {
        self.get() < other.get()
    }

    #[inline]
    fn le(&self, other: &Self) -> bool {
        self.get() <= other.get()
    }

    #[inline]
    fn gt(&self, other: &Self) -> bool {
        self.get() > other.get()
    }

    #[inline]
    fn ge(&self, other: &Self) -> bool {
        self.get() >= other.get()
    }
}

#[stable(feature = "nonzero", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<T> const Ord for NonZero<T>
where
    // FIXME(const_hack): `T: ~const Destruct` 应该能从 `Self: ~const Destruct` 推断出来。
    // 见 https://github.com/rust-lang/rust/issues/144207
    T: ZeroablePrimitive + [const] Ord + [const] Destruct,
{
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.get().cmp(&other.get())
    }

    #[inline]
    fn max(self, other: Self) -> Self {
        // SAFETY: 两个非零值的最大值仍然非零。
        unsafe { Self::new_unchecked(self.get().max(other.get())) }
    }

    #[inline]
    fn min(self, other: Self) -> Self {
        // SAFETY: 两个非零值的最小值仍然非零。
        unsafe { Self::new_unchecked(self.get().min(other.get())) }
    }

    #[inline]
    fn clamp(self, min: Self, max: Self) -> Self {
        // SAFETY: 非零值夹在两个非零边界之间后仍然非零。
        unsafe { Self::new_unchecked(self.get().clamp(min.get(), max.get())) }
    }
}

#[stable(feature = "nonzero", since = "1.28.0")]
impl<T> Hash for NonZero<T>
where
    T: ZeroablePrimitive + Hash,
{
    #[inline]
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.get().hash(state)
    }
}

#[stable(feature = "from_nonzero", since = "1.31.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<NonZero<T>> for T
where
    T: ZeroablePrimitive,
{
    #[inline]
    fn from(nonzero: NonZero<T>) -> Self {
        // 调用 `get` 方法以保留范围信息。
        nonzero.get()
    }
}

#[stable(feature = "nonzero_bitor", since = "1.45.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl<T> const BitOr for NonZero<T>
where
    T: ZeroablePrimitive + [const] BitOr<Output = T>,
{
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        // SAFETY: 两个非零值按位 OR 后仍然非零。
        unsafe { Self::new_unchecked(self.get() | rhs.get()) }
    }
}

#[stable(feature = "nonzero_bitor", since = "1.45.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl<T> const BitOr<T> for NonZero<T>
where
    T: ZeroablePrimitive + [const] BitOr<Output = T>,
{
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: T) -> Self::Output {
        // SAFETY: 非零值与任意值按位 OR 后仍然非零。
        unsafe { Self::new_unchecked(self.get() | rhs) }
    }
}

#[stable(feature = "nonzero_bitor", since = "1.45.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl<T> const BitOr<NonZero<T>> for T
where
    T: ZeroablePrimitive + [const] BitOr<Output = T>,
{
    type Output = NonZero<T>;

    #[inline]
    fn bitor(self, rhs: NonZero<T>) -> Self::Output {
        // SAFETY: 任意值与非零值按位 OR 后仍然非零。
        unsafe { NonZero::new_unchecked(self | rhs.get()) }
    }
}

#[stable(feature = "nonzero_bitor", since = "1.45.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl<T> const BitOrAssign for NonZero<T>
where
    T: ZeroablePrimitive,
    Self: [const] BitOr<Output = Self>,
{
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

#[stable(feature = "nonzero_bitor", since = "1.45.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl<T> const BitOrAssign<T> for NonZero<T>
where
    T: ZeroablePrimitive,
    Self: [const] BitOr<T, Output = Self>,
{
    #[inline]
    fn bitor_assign(&mut self, rhs: T) {
        *self = *self | rhs;
    }
}

impl<T> NonZero<T>
where
    T: ZeroablePrimitive,
{
    /// 如果给定值不为零，则创建 `NonZero`。
    #[stable(feature = "nonzero", since = "1.28.0")]
    #[rustc_const_stable(feature = "const_nonzero_int_methods", since = "1.47.0")]
    #[must_use]
    #[inline]
    pub const fn new(n: T) -> Option<Self> {
        // SAFETY: 内存布局优化保证 `Option<NonZero<T>>` 与 `T` 具有相同布局和大小，
        // 且用 `0` 表示 `None`。
        unsafe { intrinsics::transmute_unchecked(n) }
    }

    /// 不检查值是否非零，直接创建 `NonZero`。
    ///
    /// 若值为零，会导致 undefined behavior。
    ///
    /// # 安全性(Safety）
    ///
    /// `n` 必须不为零。零会破坏 `NonZero` 的类型有效性，也会让编译器基于 niche 的布局
    /// 优化假设失效。
    #[stable(feature = "nonzero", since = "1.28.0")]
    #[rustc_const_stable(feature = "nonzero", since = "1.28.0")]
    #[must_use]
    #[inline]
    #[track_caller]
    pub const unsafe fn new_unchecked(n: T) -> Self {
        match Self::new(n) {
            Some(n) => n,
            None => {
                // SAFETY: 调用方保证 `n` 非零，因此这里不可达。
                unsafe {
                    ub_checks::assert_unsafe_precondition!(
                        check_language_ub,
                        "NonZero::new_unchecked requires the argument to be non-zero",
                        () => false,
                    );
                    intrinsics::unreachable()
                }
            }
        }
    }

    /// 如果引用指向的值不为零，则把它转换为 `NonZero` 的可变引用。
    #[unstable(feature = "nonzero_from_mut", issue = "106290")]
    #[must_use]
    #[inline]
    pub fn from_mut(n: &mut T) -> Option<&mut Self> {
        // SAFETY: 内存布局优化保证 `Option<NonZero<T>>` 与 `T` 具有相同布局和大小，
        // 且用 `0` 表示 `None`。
        let opt_n = unsafe { &mut *(ptr::from_mut(n).cast::<Option<Self>>()) };

        opt_n.as_mut()
    }

    /// 不检查引用指向的值是否非零，直接把可变引用转换为 `NonZero` 的可变引用。
    ///
    /// 若引用指向零，会导致 undefined behavior。
    ///
    /// # 安全性(Safety）
    ///
    /// 被引用的值必须不为零，并且在返回的 `&mut NonZero<T>` 生命周期内不能被写成零。
    #[unstable(feature = "nonzero_from_mut", issue = "106290")]
    #[must_use]
    #[inline]
    #[track_caller]
    pub unsafe fn from_mut_unchecked(n: &mut T) -> &mut Self {
        match Self::from_mut(n) {
            Some(n) => n,
            None => {
                // SAFETY: 调用方保证 `n` 引用的值非零，因此这里不可达。
                unsafe {
                    ub_checks::assert_unsafe_precondition!(
                        check_library_ub,
                        "NonZero::from_mut_unchecked requires the argument to dereference as non-zero",
                        () => false,
                    );
                    intrinsics::unreachable()
                }
            }
        }
    }

    /// 以原始类型返回内部值。
    #[stable(feature = "nonzero", since = "1.28.0")]
    #[rustc_const_stable(feature = "const_nonzero_get", since = "1.34.0")]
    #[inline]
    pub const fn get(self) -> T {
        // rustc 只有在从某处内存加载 `self` 时，才能设置 range metadata。若 `self` 的值来自
        // 某个未内联函数的按值参数，LLVM 没有 range metadata 能理解该值不可能为零。
        //
        // 使用 transmute 会在运行时 `assume` 该范围。
        //
        // 即使 LLVM 将来支持函数参数上的 `!range` metadata
        // （见 <https://github.com/llvm/llvm-project/issues/76628>），这里也不能写成 `.0`，
        // 因为 MCP#807 禁止对 `scalar_valid_range` 类型做字段投影。并且如果这里被 MIR 内联，
        // 也没有机会再把参数 metadata 放到合适位置。
        //
        // 最终更好的方案应是 pattern types；它有望允许这里回到 `.0`，也许需要某种 cast。
        //
        // SAFETY: `ZeroablePrimitive` 保证 `.0` 的大小和位有效性满足该 transmute 的 soundness。
        unsafe { intrinsics::transmute_unchecked(self) }
    }
}

macro_rules! nonzero_integer {
    (
        #[$stability:meta]
        Self = $Ty:ident,
        Primitive = $signedness:ident $Int:ident,
        SignedPrimitive = $Sint:ty,
        UnsignedPrimitive = $Uint:ty,

        // 供 rustdoc 注释拼接示例时使用。
        rot = $rot:literal,
        rot_op = $rot_op:literal,
        rot_result = $rot_result:literal,
        swap_op = $swap_op:literal,
        swapped = $swapped:literal,
        reversed = $reversed:literal,
        leading_zeros_test = $leading_zeros_test:expr,
    ) => {
        #[doc = sign_dependent_expr!{
            $signedness ?
            if signed {
                concat!("An [`", stringify!($Int), "`] that is known not to equal zero.")
            }
            if unsigned {
                concat!("A [`", stringify!($Int), "`] that is known not to equal zero.")
            }
        }]
        ///
        /// 这会启用若干内存布局优化：编译器知道该值永远不是零，
        /// 因而零值可以作为外层枚举（例如 `Option`）的 niche。
        #[doc = concat!("For example, `Option<", stringify!($Ty), ">` is the same size as `", stringify!($Int), "`:")]
        ///
        /// ```rust
        #[doc = concat!("assert_eq!(size_of::<Option<core::num::", stringify!($Ty), ">>(), size_of::<", stringify!($Int), ">());")]
        /// ```
        ///
        /// # 布局(Layout)
        ///
        #[doc = concat!("`", stringify!($Ty), "` is guaranteed to have the same layout and bit validity as `", stringify!($Int), "`")]
        /// 唯一例外是 `0` 不是合法实例。
        #[doc = concat!("`Option<", stringify!($Ty), ">` is guaranteed to be compatible with `", stringify!($Int), "`,")]
        /// 包括在 FFI 边界上的表示兼容性。
        ///
        /// 借助 [null pointer optimization]，
        #[doc = concat!("`", stringify!($Ty), "` and `Option<", stringify!($Ty), ">`")]
        /// 保证具有相同的大小和对齐方式：
        ///
        /// ```
        #[doc = concat!("use std::num::", stringify!($Ty), ";")]
        ///
        #[doc = concat!("assert_eq!(size_of::<", stringify!($Ty), ">(), size_of::<Option<", stringify!($Ty), ">>());")]
        #[doc = concat!("assert_eq!(align_of::<", stringify!($Ty), ">(), align_of::<Option<", stringify!($Ty), ">>());")]
        /// ```
        ///
        /// # 编译期创建
        ///
        /// 因为 [`Option::unwrap()`] 和 [`Option::expect()`] 都是 `const`，
        /// 所以可以在编译期定义新的
        #[doc = concat!("`", stringify!($Ty), "`")]
        /// 值，例如：
        /// ```
        #[doc = concat!("use std::num::", stringify!($Ty), ";")]
        ///
        #[doc = concat!("const TEN: ", stringify!($Ty), " = ", stringify!($Ty) , r#"::new(10).expect("ten is non-zero");"#)]
        /// ```
        ///
        /// [null pointer optimization]: crate::option#representation
        #[$stability]
        pub type $Ty = NonZero<$Int>;

        impl NonZero<$Int> {
            /// 该非零整数类型的位宽。
            ///
            #[doc = concat!("This value is equal to [`", stringify!($Int), "::BITS`].")]
            ///
            /// # 示例
            ///
            /// ```
            /// # use std::num::NonZero;
            /// #
            #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::BITS, ", stringify!($Int), "::BITS);")]
            /// ```
            #[stable(feature = "nonzero_bits", since = "1.67.0")]
            pub const BITS: u32 = <$Int>::BITS;

            /// 返回 `self` 的二进制表示中前导零的数量。
            ///
            /// 在许多架构上，它可能比底层整数类型的 `leading_zeros()` 更快，
            /// 因为 `NonZero` 的非零不变量允许实现跳过零值特判。
            ///
            /// # 示例
            ///
            /// ```
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let n = NonZero::<", stringify!($Int), ">::new(", $leading_zeros_test, ")?;")]
            ///
            /// assert_eq!(n.leading_zeros(), 0);
            /// # Some(())
            /// # }
            /// ```
            #[stable(feature = "nonzero_leading_trailing_zeros", since = "1.53.0")]
            #[rustc_const_stable(feature = "nonzero_leading_trailing_zeros", since = "1.53.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[inline]
            pub const fn leading_zeros(self) -> u32 {
                // SAFETY: `self` 不可能为零，因此可以调用要求非零输入的 `ctlz_nonzero`。
                unsafe {
                    intrinsics::ctlz_nonzero(self.get() as $Uint)
                }
            }

            /// 返回 `self` 的二进制表示中尾随零的数量。
            ///
            /// 在许多架构上，该函数可能比底层整数类型的 `trailing_zeros()` 更快，
            /// 因为 `NonZero` 的类型不变量允许实现跳过零值特判。
            ///
            /// # 示例
            ///
            /// ```
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let n = NonZero::<", stringify!($Int), ">::new(0b0101000)?;")]
            ///
            /// assert_eq!(n.trailing_zeros(), 3);
            /// # Some(())
            /// # }
            /// ```
            #[stable(feature = "nonzero_leading_trailing_zeros", since = "1.53.0")]
            #[rustc_const_stable(feature = "nonzero_leading_trailing_zeros", since = "1.53.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[inline]
            pub const fn trailing_zeros(self) -> u32 {
                // SAFETY: `self` 不可能为零，因此可以调用要求非零输入的 `cttz_nonzero`。
                unsafe {
                    intrinsics::cttz_nonzero(self.get() as $Uint)
                }
            }

            /// 返回只保留最高有效 1 位后的 `self`。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(isolate_most_least_significant_one)]
            ///
            /// # use core::num::NonZero;
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let a = NonZero::<", stringify!($Int), ">::new(0b_01100100)?;")]
            #[doc = concat!("let b = NonZero::<", stringify!($Int), ">::new(0b_01000000)?;")]
            ///
            /// assert_eq!(a.isolate_highest_one(), b);
            /// # Some(())
            /// # }
            /// ```
            #[unstable(feature = "isolate_most_least_significant_one", issue = "136909")]
            #[must_use = "this returns the result of the operation, \
                        without modifying the original"]
            #[inline(always)]
            pub const fn isolate_highest_one(self) -> Self {
                // SAFETY:
                // `self` 非零，因此掩码只保留最高有效 1 位后仍会得到非零值。并且由于
                // 至少有一位不是零，`self.leading_zeros()` 始终小于 `$Int::BITS`。
                unsafe {
                    let bit = (((1 as $Uint) << (<$Uint>::BITS - 1)).unchecked_shr(self.leading_zeros()));
                    NonZero::new_unchecked(bit as $Int)
                }
            }

            /// 返回只保留最低有效 1 位后的 `self`。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(isolate_most_least_significant_one)]
            ///
            /// # use core::num::NonZero;
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let a = NonZero::<", stringify!($Int), ">::new(0b_01100100)?;")]
            #[doc = concat!("let b = NonZero::<", stringify!($Int), ">::new(0b_00000100)?;")]
            ///
            /// assert_eq!(a.isolate_lowest_one(), b);
            /// # Some(())
            /// # }
            /// ```
            #[unstable(feature = "isolate_most_least_significant_one", issue = "136909")]
            #[must_use = "this returns the result of the operation, \
                        without modifying the original"]
            #[inline(always)]
            pub const fn isolate_lowest_one(self) -> Self {
                let n = self.get();
                let n = n & n.wrapping_neg();

                // SAFETY: `self` 非零，因此只保留最低有效 1 位后仍然非零。
                unsafe { NonZero::new_unchecked(n) }
            }

            /// 返回 `self` 中最高 1 位的索引。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(int_lowest_highest_one)]
            ///
            /// # use core::num::NonZero;
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::new(0b1)?.highest_one(), 0);")]
            #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::new(0b1_0000)?.highest_one(), 4);")]
            #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::new(0b1_1111)?.highest_one(), 4);")]
            /// # Some(())
            /// # }
            /// ```
            #[unstable(feature = "int_lowest_highest_one", issue = "145203")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[inline(always)]
            pub const fn highest_one(self) -> u32 {
                Self::BITS - 1 - self.leading_zeros()
            }

            /// 返回 `self` 中最低 1 位的索引。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(int_lowest_highest_one)]
            ///
            /// # use core::num::NonZero;
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::new(0b1)?.lowest_one(), 0);")]
            #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::new(0b1_0000)?.lowest_one(), 4);")]
            #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::new(0b1_1111)?.lowest_one(), 0);")]
            /// # Some(())
            /// # }
            /// ```
            #[unstable(feature = "int_lowest_highest_one", issue = "145203")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[inline(always)]
            pub const fn lowest_one(self) -> u32 {
                self.trailing_zeros()
            }

            /// 返回 `self` 的二进制表示中 1 的数量。
            ///
            /// # 示例
            ///
            /// ```
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let a = NonZero::<", stringify!($Int), ">::new(0b100_0000)?;")]
            #[doc = concat!("let b = NonZero::<", stringify!($Int), ">::new(0b100_0011)?;")]
            ///
            /// assert_eq!(a.count_ones(), NonZero::new(1)?);
            /// assert_eq!(b.count_ones(), NonZero::new(3)?);
            /// # Some(())
            /// # }
            /// ```
            ///
            #[stable(feature = "non_zero_count_ones", since = "1.86.0")]
            #[rustc_const_stable(feature = "non_zero_count_ones", since = "1.86.0")]
            #[doc(alias = "popcount")]
            #[doc(alias = "popcnt")]
            #[must_use = "this returns the result of the operation, \
                        without modifying the original"]
            #[inline(always)]
            pub const fn count_ones(self) -> NonZero<u32> {
                // SAFETY:
                // `self` 非零意味着至少有一位为 1，因此 `count_ones` 的结果也非零。
                unsafe { NonZero::new_unchecked(self.get().count_ones()) }
            }

            /// 将位模式向左旋转指定数量 `n`，并把被截掉的高位绕回结果的低位。
            ///
            /// 注意这不是 `<<` 移位运算符；旋转不会丢弃位，只会改变位的位置。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(nonzero_bitwise)]
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let n = NonZero::new(", $rot_op, stringify!($Int), ")?;")]
            #[doc = concat!("let m = NonZero::new(", $rot_result, ")?;")]
            ///
            #[doc = concat!("assert_eq!(n.rotate_left(", $rot, "), m);")]
            /// # Some(())
            /// # }
            /// ```
            #[unstable(feature = "nonzero_bitwise", issue = "128281")]
            #[must_use = "this returns the result of the operation, \
                        without modifying the original"]
            #[inline(always)]
            pub const fn rotate_left(self, n: u32) -> Self {
                let result = self.get().rotate_left(n);
                // SAFETY: 旋转位模式会保留“至少有一位为 1”的性质，因此结果仍非零。
                unsafe { Self::new_unchecked(result) }
            }

            /// 将位模式向右旋转指定数量 `n`，并把被截掉的低位绕回结果的高位。
            ///
            /// 注意这不是 `>>` 移位运算符；旋转不会丢弃位，只会改变位的位置。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(nonzero_bitwise)]
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let n = NonZero::new(", $rot_result, stringify!($Int), ")?;")]
            #[doc = concat!("let m = NonZero::new(", $rot_op, ")?;")]
            ///
            #[doc = concat!("assert_eq!(n.rotate_right(", $rot, "), m);")]
            /// # Some(())
            /// # }
            /// ```
            #[unstable(feature = "nonzero_bitwise", issue = "128281")]
            #[must_use = "this returns the result of the operation, \
                        without modifying the original"]
            #[inline(always)]
            pub const fn rotate_right(self, n: u32) -> Self {
                let result = self.get().rotate_right(n);
                // SAFETY: 旋转位模式会保留“至少有一位为 1”的性质，因此结果仍非零。
                unsafe { Self::new_unchecked(result) }
            }

            /// 反转该整数的字节顺序。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(nonzero_bitwise)]
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let n = NonZero::new(", $swap_op, stringify!($Int), ")?;")]
            /// let m = n.swap_bytes();
            ///
            #[doc = concat!("assert_eq!(m, NonZero::new(", $swapped, ")?);")]
            /// # Some(())
            /// # }
            /// ```
            #[unstable(feature = "nonzero_bitwise", issue = "128281")]
            #[must_use = "this returns the result of the operation, \
                        without modifying the original"]
            #[inline(always)]
            pub const fn swap_bytes(self) -> Self {
                let result = self.get().swap_bytes();
                // SAFETY: 交换字节会保留“至少有一位为 1”的性质，因此结果仍非零。
                unsafe { Self::new_unchecked(result) }
            }

            /// 反转该整数中的位顺序。
            ///
            /// 最低有效位会变成最高有效位，次低有效位会变成次高有效位，依此类推。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(nonzero_bitwise)]
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let n = NonZero::new(", $swap_op, stringify!($Int), ")?;")]
            /// let m = n.reverse_bits();
            ///
            #[doc = concat!("assert_eq!(m, NonZero::new(", $reversed, ")?);")]
            /// # Some(())
            /// # }
            /// ```
            #[unstable(feature = "nonzero_bitwise", issue = "128281")]
            #[must_use = "this returns the result of the operation, \
                        without modifying the original"]
            #[inline(always)]
            pub const fn reverse_bits(self) -> Self {
                let result = self.get().reverse_bits();
                // SAFETY: 反转位顺序会保留“至少有一位为 1”的性质，因此结果仍非零。
                unsafe { Self::new_unchecked(result) }
            }

            /// 把一个 big endian 整数转换为目标平台字节序。
            ///
            /// 在 big endian 平台上这是 no-op；在 little endian 平台上会交换字节。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(nonzero_bitwise)]
            /// # use std::num::NonZero;
            #[doc = concat!("use std::num::", stringify!($Ty), ";")]
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let n = NonZero::new(0x1A", stringify!($Int), ")?;")]
            ///
            /// if cfg!(target_endian = "big") {
            #[doc = concat!("    assert_eq!(", stringify!($Ty), "::from_be(n), n)")]
            /// } else {
            #[doc = concat!("    assert_eq!(", stringify!($Ty), "::from_be(n), n.swap_bytes())")]
            /// }
            /// # Some(())
            /// # }
            /// ```
            #[unstable(feature = "nonzero_bitwise", issue = "128281")]
            #[must_use]
            #[inline(always)]
            pub const fn from_be(x: Self) -> Self {
                let result = $Int::from_be(x.get());
                // SAFETY: 交换字节会保留“至少有一位为 1”的性质，因此结果仍非零。
                unsafe { Self::new_unchecked(result) }
            }

            /// 把一个 little endian 整数转换为目标平台字节序。
            ///
            /// 在 little endian 平台上这是 no-op；在 big endian 平台上会交换字节。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(nonzero_bitwise)]
            /// # use std::num::NonZero;
            #[doc = concat!("use std::num::", stringify!($Ty), ";")]
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let n = NonZero::new(0x1A", stringify!($Int), ")?;")]
            ///
            /// if cfg!(target_endian = "little") {
            #[doc = concat!("    assert_eq!(", stringify!($Ty), "::from_le(n), n)")]
            /// } else {
            #[doc = concat!("    assert_eq!(", stringify!($Ty), "::from_le(n), n.swap_bytes())")]
            /// }
            /// # Some(())
            /// # }
            /// ```
            #[unstable(feature = "nonzero_bitwise", issue = "128281")]
            #[must_use]
            #[inline(always)]
            pub const fn from_le(x: Self) -> Self {
                let result = $Int::from_le(x.get());
                // SAFETY: 交换字节会保留“至少有一位为 1”的性质，因此结果仍非零。
                unsafe { Self::new_unchecked(result) }
            }

            /// 把 `self` 从目标平台字节序转换为 big endian。
            ///
            /// 在 big endian 平台上这是 no-op；在 little endian 平台上会交换字节。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(nonzero_bitwise)]
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let n = NonZero::new(0x1A", stringify!($Int), ")?;")]
            ///
            /// if cfg!(target_endian = "big") {
            ///     assert_eq!(n.to_be(), n)
            /// } else {
            ///     assert_eq!(n.to_be(), n.swap_bytes())
            /// }
            /// # Some(())
            /// # }
            /// ```
            #[unstable(feature = "nonzero_bitwise", issue = "128281")]
            #[must_use = "this returns the result of the operation, \
                        without modifying the original"]
            #[inline(always)]
            pub const fn to_be(self) -> Self {
                let result = self.get().to_be();
                // SAFETY: 交换字节会保留“至少有一位为 1”的性质，因此结果仍非零。
                unsafe { Self::new_unchecked(result) }
            }

            /// 把 `self` 从目标平台字节序转换为 little endian。
            ///
            /// 在 little endian 平台上这是 no-op；在 big endian 平台上会交换字节。
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(nonzero_bitwise)]
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let n = NonZero::new(0x1A", stringify!($Int), ")?;")]
            ///
            /// if cfg!(target_endian = "little") {
            ///     assert_eq!(n.to_le(), n)
            /// } else {
            ///     assert_eq!(n.to_le(), n.swap_bytes())
            /// }
            /// # Some(())
            /// # }
            /// ```
            #[unstable(feature = "nonzero_bitwise", issue = "128281")]
            #[must_use = "this returns the result of the operation, \
                        without modifying the original"]
            #[inline(always)]
            pub const fn to_le(self) -> Self {
                let result = self.get().to_le();
                // SAFETY: 交换字节会保留“至少有一位为 1”的性质，因此结果仍非零。
                unsafe { Self::new_unchecked(result) }
            }

            nonzero_integer_signedness_dependent_methods! {
                Primitive = $signedness $Int,
                SignedPrimitive = $Sint,
                UnsignedPrimitive = $Uint,
            }

            /// 将两个非零整数相乘。
            ///
            /// 该方法会检查溢出，并在溢出时返回 [`None`]。因此结果不会通过 wrapping 变成零。
            ///
            /// # 示例
            ///
            /// ```
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let two = NonZero::new(2", stringify!($Int), ")?;")]
            #[doc = concat!("let four = NonZero::new(4", stringify!($Int), ")?;")]
            #[doc = concat!("let max = NonZero::new(", stringify!($Int), "::MAX)?;")]
            ///
            /// assert_eq!(Some(four), two.checked_mul(two));
            /// assert_eq!(None, max.checked_mul(two));
            /// # Some(())
            /// # }
            /// ```
            #[stable(feature = "nonzero_checked_ops", since = "1.64.0")]
            #[rustc_const_stable(feature = "const_nonzero_checked_ops", since = "1.64.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[inline]
            pub const fn checked_mul(self, other: Self) -> Option<Self> {
                if let Some(result) = self.get().checked_mul(other.get()) {
                    // SAFETY:
                    // - `checked_mul` 在溢出时返回 `None`
                    // - `self` 和 `other` 都非零
                    // - 不发生溢出的乘法只有在某个操作数为零时才会得到零
                    //
                    // 因此结果不可能为零。
                    Some(unsafe { Self::new_unchecked(result) })
                } else {
                    None
                }
            }

            /// 将两个非零整数相乘。
            #[doc = concat!("Return [`NonZero::<", stringify!($Int), ">::MAX`] on overflow.")]
            ///
            /// # 示例
            ///
            /// ```
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let two = NonZero::new(2", stringify!($Int), ")?;")]
            #[doc = concat!("let four = NonZero::new(4", stringify!($Int), ")?;")]
            #[doc = concat!("let max = NonZero::new(", stringify!($Int), "::MAX)?;")]
            ///
            /// assert_eq!(four, two.saturating_mul(two));
            /// assert_eq!(max, four.saturating_mul(max));
            /// # Some(())
            /// # }
            /// ```
            #[stable(feature = "nonzero_checked_ops", since = "1.64.0")]
            #[rustc_const_stable(feature = "const_nonzero_checked_ops", since = "1.64.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[inline]
            pub const fn saturating_mul(self, other: Self) -> Self {
                    // SAFETY:
                    // - `saturating_mul` 在溢出/下溢时返回 `u*::MAX`/`i*::MAX`/`i*::MIN`，
                    //   它们全都非零
                    // - `self` 和 `other` 都非零
                    // - 不发生溢出的乘法只有在某个操作数为零时才会得到零
                    //
                    // 因此结果不可能为零。
                unsafe { Self::new_unchecked(self.get().saturating_mul(other.get())) }
            }

            /// 在假设不会溢出的前提下，将两个非零整数相乘。
            ///
            /// 溢出不会被检查；一旦溢出就是 undefined behavior，**即使 wrapping 后的结果
            /// 仍然是非零值**。当下列条件成立时，行为立即未定义：
            #[doc = sign_dependent_expr!{
                $signedness ?
                if signed {
                    concat!("`self * rhs > ", stringify!($Int), "::MAX`, ",
                            "or `self * rhs < ", stringify!($Int), "::MIN`.")
                }
                if unsigned {
                    concat!("`self * rhs > ", stringify!($Int), "::MAX`.")
                }
            }]
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(nonzero_ops)]
            ///
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let two = NonZero::new(2", stringify!($Int), ")?;")]
            #[doc = concat!("let four = NonZero::new(4", stringify!($Int), ")?;")]
            ///
            /// assert_eq!(four, unsafe { two.unchecked_mul(two) });
            /// # Some(())
            /// # }
            /// ```
            #[unstable(feature = "nonzero_ops", issue = "84186")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[inline]
            pub const unsafe fn unchecked_mul(self, other: Self) -> Self {
                // SAFETY: 调用方保证乘法不会溢出。
                unsafe { Self::new_unchecked(self.get().unchecked_mul(other.get())) }
            }

            /// 将非零值提升到整数幂。
            ///
            /// 该方法会检查溢出，并在溢出时返回 [`None`]。因此结果不会通过 wrapping 变成零。
            ///
            /// # 示例
            ///
            /// ```
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let three = NonZero::new(3", stringify!($Int), ")?;")]
            #[doc = concat!("let twenty_seven = NonZero::new(27", stringify!($Int), ")?;")]
            #[doc = concat!("let half_max = NonZero::new(", stringify!($Int), "::MAX / 2)?;")]
            ///
            /// assert_eq!(Some(twenty_seven), three.checked_pow(3));
            /// assert_eq!(None, half_max.checked_pow(3));
            /// # Some(())
            /// # }
            /// ```
            #[stable(feature = "nonzero_checked_ops", since = "1.64.0")]
            #[rustc_const_stable(feature = "const_nonzero_checked_ops", since = "1.64.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[inline]
            pub const fn checked_pow(self, other: u32) -> Option<Self> {
                if let Some(result) = self.get().checked_pow(other) {
                    // SAFETY:
                    // - `checked_pow` 在溢出/下溢时返回 `None`
                    // - `self` 非零
                    // - 不发生溢出的幂运算只有在底数为零时才会得到零
                    //
                    // 因此结果不可能为零。
                    Some(unsafe { Self::new_unchecked(result) })
                } else {
                    None
                }
            }

            /// 将非零值提升到整数幂。
            #[doc = sign_dependent_expr!{
                $signedness ?
                if signed {
                    concat!("Return [`NonZero::<", stringify!($Int), ">::MIN`] ",
                                "or [`NonZero::<", stringify!($Int), ">::MAX`] on overflow.")
                }
                if unsigned {
                    concat!("Return [`NonZero::<", stringify!($Int), ">::MAX`] on overflow.")
                }
            }]
            ///
            /// # 示例
            ///
            /// ```
            /// # use std::num::NonZero;
            /// #
            /// # fn main() { test().unwrap(); }
            /// # fn test() -> Option<()> {
            #[doc = concat!("let three = NonZero::new(3", stringify!($Int), ")?;")]
            #[doc = concat!("let twenty_seven = NonZero::new(27", stringify!($Int), ")?;")]
            #[doc = concat!("let max = NonZero::new(", stringify!($Int), "::MAX)?;")]
            ///
            /// assert_eq!(twenty_seven, three.saturating_pow(3));
            /// assert_eq!(max, max.saturating_pow(3));
            /// # Some(())
            /// # }
            /// ```
            #[stable(feature = "nonzero_checked_ops", since = "1.64.0")]
            #[rustc_const_stable(feature = "const_nonzero_checked_ops", since = "1.64.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[inline]
            pub const fn saturating_pow(self, other: u32) -> Self {
                // SAFETY:
                // - `saturating_pow` 在溢出/下溢时返回 `u*::MAX`/`i*::MAX`/`i*::MIN`，
                //   它们全都非零
                // - `self` 非零
                // - 不发生溢出的幂运算只有在底数为零时才会得到零
                //
                // 因此结果不可能为零。
                unsafe { Self::new_unchecked(self.get().saturating_pow(other)) }
            }
        }

        #[stable(feature = "nonzero_parse", since = "1.35.0")]
        impl FromStr for NonZero<$Int> {
            type Err = ParseIntError;
            fn from_str(src: &str) -> Result<Self, Self::Err> {
                Self::new(<$Int>::from_str_radix(src, 10)?)
                    .ok_or(ParseIntError {
                        kind: IntErrorKind::Zero
                    })
            }
        }

        nonzero_integer_signedness_dependent_impls!($signedness $Int);
    };

    (
        Self = $Ty:ident,
        Primitive = unsigned $Int:ident,
        SignedPrimitive = $Sint:ident,
        rot = $rot:literal,
        rot_op = $rot_op:literal,
        rot_result = $rot_result:literal,
        swap_op = $swap_op:literal,
        swapped = $swapped:literal,
        reversed = $reversed:literal,
        $(,)?
    ) => {
        nonzero_integer! {
            #[stable(feature = "nonzero", since = "1.28.0")]
            Self = $Ty,
            Primitive = unsigned $Int,
            SignedPrimitive = $Sint,
            UnsignedPrimitive = $Int,
            rot = $rot,
            rot_op = $rot_op,
            rot_result = $rot_result,
            swap_op = $swap_op,
            swapped = $swapped,
            reversed = $reversed,
            leading_zeros_test = concat!(stringify!($Int), "::MAX"),
        }
    };

    (
        Self = $Ty:ident,
        Primitive = signed $Int:ident,
        UnsignedPrimitive = $Uint:ident,
        rot = $rot:literal,
        rot_op = $rot_op:literal,
        rot_result = $rot_result:literal,
        swap_op = $swap_op:literal,
        swapped = $swapped:literal,
        reversed = $reversed:literal,
    ) => {
        nonzero_integer! {
            #[stable(feature = "signed_nonzero", since = "1.34.0")]
            Self = $Ty,
            Primitive = signed $Int,
            SignedPrimitive = $Int,
            UnsignedPrimitive = $Uint,
            rot = $rot,
            rot_op = $rot_op,
            rot_result = $rot_result,
            swap_op = $swap_op,
            swapped = $swapped,
            reversed = $reversed,
            leading_zeros_test = concat!("-1", stringify!($Int)),
        }
    };
}

macro_rules! nonzero_integer_signedness_dependent_impls {
    // 仅供无符号 NonZero 类型使用的 impl。
    (unsigned $Int:ty) => {
        #[stable(feature = "nonzero_div", since = "1.51.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Div<NonZero<$Int>> for $Int {
            type Output = $Int;

            /// 等价于 `self / other.get()`；但因为 `other` 是 `NonZero<_>`，
            /// 除数不可能为零，所以运行时不需要再做除零检查。
            ///
            /// 该操作向零舍入，截断精确结果的小数部分，并且不会 panic。
            #[doc(alias = "unchecked_div")]
            #[inline]
            fn div(self, other: NonZero<$Int>) -> $Int {
                // SAFETY: `other` 的非零不变量排除了除零；`self` 是无符号整数，
                // 因而不存在有符号 `MIN / -1` 的溢出情形。
                unsafe { intrinsics::unchecked_div(self, other.get()) }
            }
        }

        #[stable(feature = "nonzero_div_assign", since = "1.79.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const DivAssign<NonZero<$Int>> for $Int {
            /// 等价于 `self /= other.get()`；但因为 `other` 是 `NonZero<_>`，
            /// 除数不可能为零，所以运行时不需要再做除零检查。
            ///
            /// 该操作向零舍入，截断精确结果的小数部分，并且不会 panic。
            #[inline]
            fn div_assign(&mut self, other: NonZero<$Int>) {
                *self = *self / other;
            }
        }

        #[stable(feature = "nonzero_div", since = "1.51.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Rem<NonZero<$Int>> for $Int {
            type Output = $Int;

            /// 该操作满足 `n % d == n - (n / d) * d`，并且不会 panic。
            #[inline]
            fn rem(self, other: NonZero<$Int>) -> $Int {
                // SAFETY: `other` 的非零不变量排除了对零取余；`self` 是无符号整数，
                // 因而不存在有符号 `MIN % -1` 的溢出情形。
                unsafe { intrinsics::unchecked_rem(self, other.get()) }
            }
        }

        #[stable(feature = "nonzero_div_assign", since = "1.79.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const RemAssign<NonZero<$Int>> for $Int {
            /// 该操作满足 `n % d == n - (n / d) * d`，并且不会 panic。
            #[inline]
            fn rem_assign(&mut self, other: NonZero<$Int>) {
                *self = *self % other;
            }
        }

        impl NonZero<$Int> {
            /// 计算 `self` 除以 `rhs` 的商，并向正无穷方向舍入。
            ///
            /// 两个操作数都为正的 `NonZero` 值，因此结果保证非零。
            ///
            /// # 示例
            ///
            /// ```
            /// # use std::num::NonZero;
            #[doc = concat!("let one = NonZero::new(1", stringify!($Int), ").unwrap();")]
            #[doc = concat!("let max = NonZero::new(", stringify!($Int), "::MAX).unwrap();")]
            /// assert_eq!(one.div_ceil(max), one);
            ///
            #[doc = concat!("let two = NonZero::new(2", stringify!($Int), ").unwrap();")]
            #[doc = concat!("let three = NonZero::new(3", stringify!($Int), ").unwrap();")]
            /// assert_eq!(three.div_ceil(two), two);
            /// ```
            #[stable(feature = "unsigned_nonzero_div_ceil", since = "1.92.0")]
            #[rustc_const_stable(feature = "unsigned_nonzero_div_ceil", since = "1.92.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[inline]
            pub const fn div_ceil(self, rhs: Self) -> Self {
                let v = self.get().div_ceil(rhs.get());
                // SAFETY: 两个正整数的向上取整除法结果不可能为零。
                unsafe { Self::new_unchecked(v) }
            }
        }
    };
    // 仅供有符号 NonZero 类型使用的 impl。
    (signed $Int:ty) => {
        #[stable(feature = "signed_nonzero_neg", since = "1.71.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Neg for NonZero<$Int> {
            type Output = Self;

            #[inline]
            fn neg(self) -> Self {
                // SAFETY: 非零值取负仍不可能产生零。
                unsafe { Self::new_unchecked(self.get().neg()) }
            }
        }

        forward_ref_unop! { impl Neg, neg for NonZero<$Int>,
        #[stable(feature = "signed_nonzero_neg", since = "1.71.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    };
}

#[rustfmt::skip] // https://github.com/rust-lang/rustfmt/issues/5974
macro_rules! nonzero_integer_signedness_dependent_methods {
    // 仅供无符号 NonZero 类型使用的关联项。
    (
        Primitive = unsigned $Int:ident,
        SignedPrimitive = $Sint:ty,
        UnsignedPrimitive = $Uint:ty,
    ) => {
        /// 该非零整数类型可以表示的最小值，即 1。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::MIN.get(), 1", stringify!($Int), ");")]
        /// ```
        #[stable(feature = "nonzero_min_max", since = "1.70.0")]
        pub const MIN: Self = Self::new(1).unwrap();

        /// 该非零整数类型可以表示的最大值，
        #[doc = concat!("equal to [`", stringify!($Int), "::MAX`].")]
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::MAX.get(), ", stringify!($Int), "::MAX);")]
        /// ```
        #[stable(feature = "nonzero_min_max", since = "1.70.0")]
        pub const MAX: Self = Self::new(<$Int>::MAX).unwrap();

        /// 将无符号整数加到一个非零值上。
        /// 该 `checked` 变体会检测溢出，并在溢出时返回 [`None`]。
        /// 因为溢出不会被包裹回类型范围内，所以结果也不会绕回到零。
        ///
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let one = NonZero::new(1", stringify!($Int), ")?;")]
        #[doc = concat!("let two = NonZero::new(2", stringify!($Int), ")?;")]
        #[doc = concat!("let max = NonZero::new(", stringify!($Int), "::MAX)?;")]
        ///
        /// assert_eq!(Some(two), one.checked_add(1));
        /// assert_eq!(None, max.checked_add(1));
        /// # Some(())
        /// # }
        /// ```
        #[stable(feature = "nonzero_checked_ops", since = "1.64.0")]
        #[rustc_const_stable(feature = "const_nonzero_checked_ops", since = "1.64.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_add(self, other: $Int) -> Option<Self> {
            if let Some(result) = self.get().checked_add(other) {
                // SAFETY:
                // - `checked_add` 在溢出时返回 `None`
                // - `self` 是非零值
                // - 未溢出的加法只有在两边都为零时才会得到零
                //
                // 因此结果不可能为零。
                Some(unsafe { Self::new_unchecked(result) })
            } else {
                None
            }
        }

        /// 将无符号整数加到一个非零值上。
        #[doc = concat!("Return [`NonZero::<", stringify!($Int), ">::MAX`] on overflow.")]
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let one = NonZero::new(1", stringify!($Int), ")?;")]
        #[doc = concat!("let two = NonZero::new(2", stringify!($Int), ")?;")]
        #[doc = concat!("let max = NonZero::new(", stringify!($Int), "::MAX)?;")]
        ///
        /// assert_eq!(two, one.saturating_add(1));
        /// assert_eq!(max, max.saturating_add(1));
        /// # Some(())
        /// # }
        /// ```
        #[stable(feature = "nonzero_checked_ops", since = "1.64.0")]
        #[rustc_const_stable(feature = "const_nonzero_checked_ops", since = "1.64.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn saturating_add(self, other: $Int) -> Self {
            // SAFETY:
            // - `saturating_add` 在溢出时返回非零的 `u*::MAX`
            // - `self` 是非零值
            // - 未溢出的加法只有在两边都为零时才会得到零
            //
            // 因此结果不可能为零。
            unsafe { Self::new_unchecked(self.get().saturating_add(other)) }
        }

        /// 将无符号整数加到一个非零值上，并假定不会发生溢出。
        /// 这是 `unchecked` 变体：一旦加法溢出就是未定义行为，
        /// *即使按二进制补码包裹后的结果仍然是非零值*。
        /// 只要满足以下条件，行为就已经未定义：
        #[doc = concat!("`self + rhs > ", stringify!($Int), "::MAX`.")]
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(nonzero_ops)]
        ///
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let one = NonZero::new(1", stringify!($Int), ")?;")]
        #[doc = concat!("let two = NonZero::new(2", stringify!($Int), ")?;")]
        ///
        /// assert_eq!(two, unsafe { one.unchecked_add(1) });
        /// # Some(())
        /// # }
        /// ```
        #[unstable(feature = "nonzero_ops", issue = "84186")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const unsafe fn unchecked_add(self, other: $Int) -> Self {
            // SAFETY: 调用方保证不会发生溢出。
            unsafe { Self::new_unchecked(self.get().unchecked_add(other)) }
        }

        /// 返回大于或等于 `self` 的最小二的幂。
        /// 该 `checked` 变体会检测溢出；如果下一个二的幂超过该类型的最大值，
        /// 则返回 [`None`]。因此结果不会因溢出而包裹回零。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let two = NonZero::new(2", stringify!($Int), ")?;")]
        #[doc = concat!("let three = NonZero::new(3", stringify!($Int), ")?;")]
        #[doc = concat!("let four = NonZero::new(4", stringify!($Int), ")?;")]
        #[doc = concat!("let max = NonZero::new(", stringify!($Int), "::MAX)?;")]
        ///
        /// assert_eq!(Some(two), two.checked_next_power_of_two() );
        /// assert_eq!(Some(four), three.checked_next_power_of_two() );
        /// assert_eq!(None, max.checked_next_power_of_two() );
        /// # Some(())
        /// # }
        /// ```
        #[stable(feature = "nonzero_checked_ops", since = "1.64.0")]
        #[rustc_const_stable(feature = "const_nonzero_checked_ops", since = "1.64.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_next_power_of_two(self) -> Option<Self> {
            if let Some(nz) = self.get().checked_next_power_of_two() {
                // SAFETY: 下一个二的幂为正，并且溢出已经被 `checked` 路径排除。
                Some(unsafe { Self::new_unchecked(nz) })
            } else {
                None
            }
        }

        /// 返回该数以 2 为底的对数，并向下取整。
        ///
        /// 这与
        #[doc = concat!("[`", stringify!($Int), "::ilog2`],")]
        /// 是同一操作；不同之处是该值永远不是零，因此无需处理零输入的失败情形。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("assert_eq!(NonZero::new(7", stringify!($Int), ")?.ilog2(), 2);")]
        #[doc = concat!("assert_eq!(NonZero::new(8", stringify!($Int), ")?.ilog2(), 3);")]
        #[doc = concat!("assert_eq!(NonZero::new(9", stringify!($Int), ")?.ilog2(), 3);")]
        /// # Some(())
        /// # }
        /// ```
        #[stable(feature = "int_log", since = "1.67.0")]
        #[rustc_const_stable(feature = "int_log", since = "1.67.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn ilog2(self) -> u32 {
            Self::BITS - 1 - self.leading_zeros()
        }

        /// 返回该数以 10 为底的对数，并向下取整。
        ///
        /// 这与
        #[doc = concat!("[`", stringify!($Int), "::ilog10`],")]
        /// 是同一操作；不同之处是该值永远不是零，因此无需处理零输入的失败情形。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("assert_eq!(NonZero::new(99", stringify!($Int), ")?.ilog10(), 1);")]
        #[doc = concat!("assert_eq!(NonZero::new(100", stringify!($Int), ")?.ilog10(), 2);")]
        #[doc = concat!("assert_eq!(NonZero::new(101", stringify!($Int), ")?.ilog10(), 2);")]
        /// # Some(())
        /// # }
        /// ```
        #[stable(feature = "int_log", since = "1.67.0")]
        #[rustc_const_stable(feature = "int_log", since = "1.67.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn ilog10(self) -> u32 {
            super::int_log10::$Int(self)
        }

        /// 计算 `self` 与 `rhs` 的中点（平均值）。
        ///
        /// `midpoint(a, b)` 的语义等同于在足够大的有符号整数类型中计算
        /// `(a + b) >> 1`。这意味着结果总是向负无穷方向舍入，并且不会发生溢出。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let one = NonZero::new(1", stringify!($Int), ")?;")]
        #[doc = concat!("let two = NonZero::new(2", stringify!($Int), ")?;")]
        #[doc = concat!("let four = NonZero::new(4", stringify!($Int), ")?;")]
        ///
        /// assert_eq!(one.midpoint(four), two);
        /// assert_eq!(four.midpoint(one), two);
        /// # Some(())
        /// # }
        /// ```
        #[stable(feature = "num_midpoint", since = "1.85.0")]
        #[rustc_const_stable(feature = "num_midpoint", since = "1.85.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[doc(alias = "average_floor")]
        #[doc(alias = "average")]
        #[inline]
        pub const fn midpoint(self, rhs: Self) -> Self {
            // SAFETY: 中点为 `0` 只可能来自互为相反数或近似相反的输入，
            // 例如 (-5, 5)、(0, 1)、(0, 0)。这里的类型是无符号的，
            // 且 `Self` 保证永远不是 0，因此这些情形不可能出现。
            unsafe { Self::new_unchecked(self.get().midpoint(rhs.get())) }
        }

        /// 当且仅当存在某个 `k` 使得 `self == (1 << k)` 时返回 `true`。
        ///
        /// 在许多架构上，它可能比底层整数类型的 `is_power_of_two()` 更快，
        /// 因为 `NonZero` 的非零不变量允许实现跳过零值特判。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let eight = NonZero::new(8", stringify!($Int), ")?;")]
        /// assert!(eight.is_power_of_two());
        #[doc = concat!("let ten = NonZero::new(10", stringify!($Int), ")?;")]
        /// assert!(!ten.is_power_of_two());
        /// # Some(())
        /// # }
        /// ```
        #[must_use]
        #[stable(feature = "nonzero_is_power_of_two", since = "1.59.0")]
        #[rustc_const_stable(feature = "nonzero_is_power_of_two", since = "1.59.0")]
        #[inline]
        pub const fn is_power_of_two(self) -> bool {
            // LLVM 11 会把 `unchecked_sub(x, 1) & x == 0` 规范化为这里看到的实现。
            // 在基础 x86-64 目标上，跳过零检查可节省 3 条指令。
            // 在带 BMI1 的 x86_64 上，非零前提让它能生成 `BLSR`，
            // 相比底层整数类型上的 `POPCNT` 实现还能少一条指令。

            intrinsics::ctpop(self.get()) < 2
        }

        /// 返回该数的平方根，并向下取整。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let ten = NonZero::new(10", stringify!($Int), ")?;")]
        #[doc = concat!("let three = NonZero::new(3", stringify!($Int), ")?;")]
        ///
        /// assert_eq!(ten.isqrt(), three);
        /// # Some(())
        /// # }
        /// ```
        #[stable(feature = "isqrt", since = "1.84.0")]
        #[rustc_const_stable(feature = "isqrt", since = "1.84.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn isqrt(self) -> Self {
            let result = self.get().isqrt();

            // SAFETY: 整数平方根是单调不减函数；增大输入不会让输出变小。
            // 无符号 NonZero 输入的下界为 1，因此结果下界是 sqrt(1)，也就是 1，
            // 所以结果不可能为零。
            unsafe { Self::new_unchecked(result) }
        }

        /// 返回把 `self` 的位模式重新解释为同宽有符号整数后的结果。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        ///
        #[doc = concat!("let n = NonZero::<", stringify!($Int), ">::MAX;")]
        ///
        #[doc = concat!("assert_eq!(n.cast_signed(), NonZero::new(-1", stringify!($Sint), ").unwrap());")]
        /// ```
        #[stable(feature = "integer_sign_cast", since = "1.87.0")]
        #[rustc_const_stable(feature = "integer_sign_cast", since = "1.87.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn cast_signed(self) -> NonZero<$Sint> {
            // SAFETY: `self.get()` 不可能为零。
            unsafe { NonZero::new_unchecked(self.get().cast_signed()) }
        }

        /// 返回表示 `self` 所需的最少位数。
        ///
        /// # 示例
        ///
        /// ```
        /// #![feature(uint_bit_width)]
        ///
        /// # use core::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::MIN.bit_width(), NonZero::new(1)?);")]
        #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::new(0b111)?.bit_width(), NonZero::new(3)?);")]
        #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::new(0b1110)?.bit_width(), NonZero::new(4)?);")]
        /// # Some(())
        /// # }
        /// ```
        #[unstable(feature = "uint_bit_width", issue = "142326")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn bit_width(self) -> NonZero<u32> {
            // SAFETY: `self.leading_zeros()` 总是小于 `Self::BITS`，
            // 因此该减法结果不可能为零。
            unsafe { NonZero::new_unchecked(Self::BITS - self.leading_zeros()) }
        }
    };

    // 仅供有符号 NonZero 类型使用的关联项。
    (
        Primitive = signed $Int:ident,
        SignedPrimitive = $Sint:ty,
        UnsignedPrimitive = $Uint:ty,
    ) => {
        /// 该非零整数类型可以表示的最小值，
        #[doc = concat!("equal to [`", stringify!($Int), "::MIN`].")]
        ///
        /// 注意：大多数整数类型能表示 `MIN` 与 `MAX` 之间的每个整数；
        /// 有符号非零整数是特例，它们在 0 处有一个不可表示的“空洞”。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::MIN.get(), ", stringify!($Int), "::MIN);")]
        /// ```
        #[stable(feature = "nonzero_min_max", since = "1.70.0")]
        pub const MIN: Self = Self::new(<$Int>::MIN).unwrap();

        /// 该非零整数类型可以表示的最大值，
        #[doc = concat!("equal to [`", stringify!($Int), "::MAX`].")]
        ///
        /// 注意：大多数整数类型能表示 `MIN` 与 `MAX` 之间的每个整数；
        /// 有符号非零整数是特例，它们在 0 处有一个不可表示的“空洞”。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        #[doc = concat!("assert_eq!(NonZero::<", stringify!($Int), ">::MAX.get(), ", stringify!($Int), "::MAX);")]
        /// ```
        #[stable(feature = "nonzero_min_max", since = "1.70.0")]
        pub const MAX: Self = Self::new(<$Int>::MAX).unwrap();

        /// 计算 `self` 的绝对值。
        #[doc = concat!("See [`", stringify!($Int), "::abs`]")]
        /// 了解溢出行为的详细说明。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let pos = NonZero::new(1", stringify!($Int), ")?;")]
        #[doc = concat!("let neg = NonZero::new(-1", stringify!($Int), ")?;")]
        ///
        /// assert_eq!(pos, pos.abs());
        /// assert_eq!(pos, neg.abs());
        /// # Some(())
        /// # }
        /// ```
        #[stable(feature = "nonzero_checked_ops", since = "1.64.0")]
        #[rustc_const_stable(feature = "const_nonzero_checked_ops", since = "1.64.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn abs(self) -> Self {
            // SAFETY: 该操作即使溢出也不会得到零。
            unsafe { Self::new_unchecked(self.get().abs()) }
        }

        /// `checked` 绝对值。
        /// 该变体会检测溢出，并在以下条件成立时返回 [`None`]：
        #[doc = concat!("`self == NonZero::<", stringify!($Int), ">::MIN`.")]
        /// 只要返回 [`Some`]，结果就不可能为零。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let pos = NonZero::new(1", stringify!($Int), ")?;")]
        #[doc = concat!("let neg = NonZero::new(-1", stringify!($Int), ")?;")]
        #[doc = concat!("let min = NonZero::new(", stringify!($Int), "::MIN)?;")]
        ///
        /// assert_eq!(Some(pos), neg.checked_abs());
        /// assert_eq!(None, min.checked_abs());
        /// # Some(())
        /// # }
        /// ```
        #[stable(feature = "nonzero_checked_ops", since = "1.64.0")]
        #[rustc_const_stable(feature = "const_nonzero_checked_ops", since = "1.64.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn checked_abs(self) -> Option<Self> {
            if let Some(nz) = self.get().checked_abs() {
                // SAFETY: 非零值的绝对值不可能产生零。
                Some(unsafe { Self::new_unchecked(nz) })
            } else {
                None
            }
        }

        /// 计算 `self` 的绝对值，并同时返回溢出信息；参见
        #[doc = concat!("[`", stringify!($Int), "::overflowing_abs`].")]
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let pos = NonZero::new(1", stringify!($Int), ")?;")]
        #[doc = concat!("let neg = NonZero::new(-1", stringify!($Int), ")?;")]
        #[doc = concat!("let min = NonZero::new(", stringify!($Int), "::MIN)?;")]
        ///
        /// assert_eq!((pos, false), pos.overflowing_abs());
        /// assert_eq!((pos, false), neg.overflowing_abs());
        /// assert_eq!((min, true), min.overflowing_abs());
        /// # Some(())
        /// # }
        /// ```
        #[stable(feature = "nonzero_checked_ops", since = "1.64.0")]
        #[rustc_const_stable(feature = "const_nonzero_checked_ops", since = "1.64.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn overflowing_abs(self) -> (Self, bool) {
            let (nz, flag) = self.get().overflowing_abs();
            (
                // SAFETY: 非零值的绝对值不可能产生零。
                unsafe { Self::new_unchecked(nz) },
                flag,
            )
        }

        /// `saturating` 绝对值；参见
        #[doc = concat!("[`", stringify!($Int), "::saturating_abs`].")]
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let pos = NonZero::new(1", stringify!($Int), ")?;")]
        #[doc = concat!("let neg = NonZero::new(-1", stringify!($Int), ")?;")]
        #[doc = concat!("let min = NonZero::new(", stringify!($Int), "::MIN)?;")]
        #[doc = concat!("let min_plus = NonZero::new(", stringify!($Int), "::MIN + 1)?;")]
        #[doc = concat!("let max = NonZero::new(", stringify!($Int), "::MAX)?;")]
        ///
        /// assert_eq!(pos, pos.saturating_abs());
        /// assert_eq!(pos, neg.saturating_abs());
        /// assert_eq!(max, min.saturating_abs());
        /// assert_eq!(max, min_plus.saturating_abs());
        /// # Some(())
        /// # }
        /// ```
        #[stable(feature = "nonzero_checked_ops", since = "1.64.0")]
        #[rustc_const_stable(feature = "const_nonzero_checked_ops", since = "1.64.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn saturating_abs(self) -> Self {
            // SAFETY: 非零值的绝对值不可能产生零。
            unsafe { Self::new_unchecked(self.get().saturating_abs()) }
        }

        /// `wrapping` 绝对值；参见
        #[doc = concat!("[`", stringify!($Int), "::wrapping_abs`].")]
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let pos = NonZero::new(1", stringify!($Int), ")?;")]
        #[doc = concat!("let neg = NonZero::new(-1", stringify!($Int), ")?;")]
        #[doc = concat!("let min = NonZero::new(", stringify!($Int), "::MIN)?;")]
        #[doc = concat!("# let max = NonZero::new(", stringify!($Int), "::MAX)?;")]
        ///
        /// assert_eq!(pos, pos.wrapping_abs());
        /// assert_eq!(pos, neg.wrapping_abs());
        /// assert_eq!(min, min.wrapping_abs());
        /// assert_eq!(max, (-max).wrapping_abs());
        /// # Some(())
        /// # }
        /// ```
        #[stable(feature = "nonzero_checked_ops", since = "1.64.0")]
        #[rustc_const_stable(feature = "const_nonzero_checked_ops", since = "1.64.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn wrapping_abs(self) -> Self {
            // SAFETY: 非零值的绝对值不可能产生零。
            unsafe { Self::new_unchecked(self.get().wrapping_abs()) }
        }

        /// 以无符号类型计算 `self` 的绝对值，
        /// 不会发生 `wrapping`，也不会 panic。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let u_pos = NonZero::new(1", stringify!($Uint), ")?;")]
        #[doc = concat!("let i_pos = NonZero::new(1", stringify!($Int), ")?;")]
        #[doc = concat!("let i_neg = NonZero::new(-1", stringify!($Int), ")?;")]
        #[doc = concat!("let i_min = NonZero::new(", stringify!($Int), "::MIN)?;")]
        #[doc = concat!("let u_max = NonZero::new(", stringify!($Uint), "::MAX / 2 + 1)?;")]
        ///
        /// assert_eq!(u_pos, i_pos.unsigned_abs());
        /// assert_eq!(u_pos, i_neg.unsigned_abs());
        /// assert_eq!(u_max, i_min.unsigned_abs());
        /// # Some(())
        /// # }
        /// ```
        #[stable(feature = "nonzero_checked_ops", since = "1.64.0")]
        #[rustc_const_stable(feature = "const_nonzero_checked_ops", since = "1.64.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline]
        pub const fn unsigned_abs(self) -> NonZero<$Uint> {
            // SAFETY: 非零值的绝对值不可能产生零。
            unsafe { NonZero::new_unchecked(self.get().unsigned_abs()) }
        }

        /// 如果 `self` 为正则返回 `true`，如果该数为负则返回 `false`。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let pos_five = NonZero::new(5", stringify!($Int), ")?;")]
        #[doc = concat!("let neg_five = NonZero::new(-5", stringify!($Int), ")?;")]
        ///
        /// assert!(pos_five.is_positive());
        /// assert!(!neg_five.is_positive());
        /// # Some(())
        /// # }
        /// ```
        #[must_use]
        #[inline]
        #[stable(feature = "nonzero_negation_ops", since = "1.71.0")]
        #[rustc_const_stable(feature = "nonzero_negation_ops", since = "1.71.0")]
        pub const fn is_positive(self) -> bool {
            self.get().is_positive()
        }

        /// 如果 `self` 为负则返回 `true`，如果该数为正则返回 `false`。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let pos_five = NonZero::new(5", stringify!($Int), ")?;")]
        #[doc = concat!("let neg_five = NonZero::new(-5", stringify!($Int), ")?;")]
        ///
        /// assert!(neg_five.is_negative());
        /// assert!(!pos_five.is_negative());
        /// # Some(())
        /// # }
        /// ```
        #[must_use]
        #[inline]
        #[stable(feature = "nonzero_negation_ops", since = "1.71.0")]
        #[rustc_const_stable(feature = "nonzero_negation_ops", since = "1.71.0")]
        pub const fn is_negative(self) -> bool {
            self.get().is_negative()
        }

        /// `checked` 取负。计算 `-self`，
        #[doc = concat!("returning `None` if `self == NonZero::<", stringify!($Int), ">::MIN`.")]
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let pos_five = NonZero::new(5", stringify!($Int), ")?;")]
        #[doc = concat!("let neg_five = NonZero::new(-5", stringify!($Int), ")?;")]
        #[doc = concat!("let min = NonZero::new(", stringify!($Int), "::MIN)?;")]
        ///
        /// assert_eq!(pos_five.checked_neg(), Some(neg_five));
        /// assert_eq!(min.checked_neg(), None);
        /// # Some(())
        /// # }
        /// ```
        #[inline]
        #[stable(feature = "nonzero_negation_ops", since = "1.71.0")]
        #[rustc_const_stable(feature = "nonzero_negation_ops", since = "1.71.0")]
        pub const fn checked_neg(self) -> Option<Self> {
            if let Some(result) = self.get().checked_neg() {
                // SAFETY: 非零值取负仍不可能产生零。
                return Some(unsafe { Self::new_unchecked(result) });
            }
            None
        }

        /// 对 `self` 取负；当它等于最小值时标记为溢出。
        ///
        #[doc = concat!("See [`", stringify!($Int), "::overflowing_neg`]")]
        /// 了解溢出行为的详细说明。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let pos_five = NonZero::new(5", stringify!($Int), ")?;")]
        #[doc = concat!("let neg_five = NonZero::new(-5", stringify!($Int), ")?;")]
        #[doc = concat!("let min = NonZero::new(", stringify!($Int), "::MIN)?;")]
        ///
        /// assert_eq!(pos_five.overflowing_neg(), (neg_five, false));
        /// assert_eq!(min.overflowing_neg(), (min, true));
        /// # Some(())
        /// # }
        /// ```
        #[inline]
        #[stable(feature = "nonzero_negation_ops", since = "1.71.0")]
        #[rustc_const_stable(feature = "nonzero_negation_ops", since = "1.71.0")]
        pub const fn overflowing_neg(self) -> (Self, bool) {
            let (result, overflow) = self.get().overflowing_neg();
            // SAFETY: 非零值取负仍不可能产生零。
            ((unsafe { Self::new_unchecked(result) }), overflow)
        }

        /// `saturating` 取负。计算 `-self`，
        #[doc = concat!("returning [`NonZero::<", stringify!($Int), ">::MAX`]")]
        #[doc = concat!("if `self == NonZero::<", stringify!($Int), ">::MIN`")]
        /// 以避免溢出。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let pos_five = NonZero::new(5", stringify!($Int), ")?;")]
        #[doc = concat!("let neg_five = NonZero::new(-5", stringify!($Int), ")?;")]
        #[doc = concat!("let min = NonZero::new(", stringify!($Int), "::MIN)?;")]
        #[doc = concat!("let min_plus_one = NonZero::new(", stringify!($Int), "::MIN + 1)?;")]
        #[doc = concat!("let max = NonZero::new(", stringify!($Int), "::MAX)?;")]
        ///
        /// assert_eq!(pos_five.saturating_neg(), neg_five);
        /// assert_eq!(min.saturating_neg(), max);
        /// assert_eq!(max.saturating_neg(), min_plus_one);
        /// # Some(())
        /// # }
        /// ```
        #[inline]
        #[stable(feature = "nonzero_negation_ops", since = "1.71.0")]
        #[rustc_const_stable(feature = "nonzero_negation_ops", since = "1.71.0")]
        pub const fn saturating_neg(self) -> Self {
            if let Some(result) = self.checked_neg() {
                return result;
            }
            Self::MAX
        }

        /// `wrapping`（模运算）取负。计算 `-self`，
        /// 并在类型边界处按模意义包裹。
        ///
        #[doc = concat!("See [`", stringify!($Int), "::wrapping_neg`]")]
        /// 了解溢出行为的详细说明。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        /// #
        /// # fn main() { test().unwrap(); }
        /// # fn test() -> Option<()> {
        #[doc = concat!("let pos_five = NonZero::new(5", stringify!($Int), ")?;")]
        #[doc = concat!("let neg_five = NonZero::new(-5", stringify!($Int), ")?;")]
        #[doc = concat!("let min = NonZero::new(", stringify!($Int), "::MIN)?;")]
        ///
        /// assert_eq!(pos_five.wrapping_neg(), neg_five);
        /// assert_eq!(min.wrapping_neg(), min);
        /// # Some(())
        /// # }
        /// ```
        #[inline]
        #[stable(feature = "nonzero_negation_ops", since = "1.71.0")]
        #[rustc_const_stable(feature = "nonzero_negation_ops", since = "1.71.0")]
        pub const fn wrapping_neg(self) -> Self {
            let result = self.get().wrapping_neg();
            // SAFETY: 非零值取负仍不可能产生零。
            unsafe { Self::new_unchecked(result) }
        }

        /// 返回把 `self` 的位模式重新解释为同宽无符号整数后的结果。
        ///
        /// # 示例
        ///
        /// ```
        /// # use std::num::NonZero;
        ///
        #[doc = concat!("let n = NonZero::new(-1", stringify!($Int), ").unwrap();")]
        ///
        #[doc = concat!("assert_eq!(n.cast_unsigned(), NonZero::<", stringify!($Uint), ">::MAX);")]
        /// ```
        #[stable(feature = "integer_sign_cast", since = "1.87.0")]
        #[rustc_const_stable(feature = "integer_sign_cast", since = "1.87.0")]
        #[must_use = "this returns the result of the operation, \
                      without modifying the original"]
        #[inline(always)]
        pub const fn cast_unsigned(self) -> NonZero<$Uint> {
            // SAFETY: `self.get()` 不可能为零。
            unsafe { NonZero::new_unchecked(self.get().cast_unsigned()) }
        }

    };
}

nonzero_integer! {
    Self = NonZeroU8,
    Primitive = unsigned u8,
    SignedPrimitive = i8,
    rot = 2,
    rot_op = "0x82",
    rot_result = "0xa",
    swap_op = "0x12",
    swapped = "0x12",
    reversed = "0x48",
}

nonzero_integer! {
    Self = NonZeroU16,
    Primitive = unsigned u16,
    SignedPrimitive = i16,
    rot = 4,
    rot_op = "0xa003",
    rot_result = "0x3a",
    swap_op = "0x1234",
    swapped = "0x3412",
    reversed = "0x2c48",
}

nonzero_integer! {
    Self = NonZeroU32,
    Primitive = unsigned u32,
    SignedPrimitive = i32,
    rot = 8,
    rot_op = "0x10000b3",
    rot_result = "0xb301",
    swap_op = "0x12345678",
    swapped = "0x78563412",
    reversed = "0x1e6a2c48",
}

nonzero_integer! {
    Self = NonZeroU64,
    Primitive = unsigned u64,
    SignedPrimitive = i64,
    rot = 12,
    rot_op = "0xaa00000000006e1",
    rot_result = "0x6e10aa",
    swap_op = "0x1234567890123456",
    swapped = "0x5634129078563412",
    reversed = "0x6a2c48091e6a2c48",
}

nonzero_integer! {
    Self = NonZeroU128,
    Primitive = unsigned u128,
    SignedPrimitive = i128,
    rot = 16,
    rot_op = "0x13f40000000000000000000000004f76",
    rot_result = "0x4f7613f4",
    swap_op = "0x12345678901234567890123456789012",
    swapped = "0x12907856341290785634129078563412",
    reversed = "0x48091e6a2c48091e6a2c48091e6a2c48",
}

#[cfg(target_pointer_width = "16")]
nonzero_integer! {
    Self = NonZeroUsize,
    Primitive = unsigned usize,
    SignedPrimitive = isize,
    rot = 4,
    rot_op = "0xa003",
    rot_result = "0x3a",
    swap_op = "0x1234",
    swapped = "0x3412",
    reversed = "0x2c48",
}

#[cfg(target_pointer_width = "32")]
nonzero_integer! {
    Self = NonZeroUsize,
    Primitive = unsigned usize,
    SignedPrimitive = isize,
    rot = 8,
    rot_op = "0x10000b3",
    rot_result = "0xb301",
    swap_op = "0x12345678",
    swapped = "0x78563412",
    reversed = "0x1e6a2c48",
}

#[cfg(target_pointer_width = "64")]
nonzero_integer! {
    Self = NonZeroUsize,
    Primitive = unsigned usize,
    SignedPrimitive = isize,
    rot = 12,
    rot_op = "0xaa00000000006e1",
    rot_result = "0x6e10aa",
    swap_op = "0x1234567890123456",
    swapped = "0x5634129078563412",
    reversed = "0x6a2c48091e6a2c48",
}

nonzero_integer! {
    Self = NonZeroI8,
    Primitive = signed i8,
    UnsignedPrimitive = u8,
    rot = 2,
    rot_op = "-0x7e",
    rot_result = "0xa",
    swap_op = "0x12",
    swapped = "0x12",
    reversed = "0x48",
}

nonzero_integer! {
    Self = NonZeroI16,
    Primitive = signed i16,
    UnsignedPrimitive = u16,
    rot = 4,
    rot_op = "-0x5ffd",
    rot_result = "0x3a",
    swap_op = "0x1234",
    swapped = "0x3412",
    reversed = "0x2c48",
}

nonzero_integer! {
    Self = NonZeroI32,
    Primitive = signed i32,
    UnsignedPrimitive = u32,
    rot = 8,
    rot_op = "0x10000b3",
    rot_result = "0xb301",
    swap_op = "0x12345678",
    swapped = "0x78563412",
    reversed = "0x1e6a2c48",
}

nonzero_integer! {
    Self = NonZeroI64,
    Primitive = signed i64,
    UnsignedPrimitive = u64,
    rot = 12,
    rot_op = "0xaa00000000006e1",
    rot_result = "0x6e10aa",
    swap_op = "0x1234567890123456",
    swapped = "0x5634129078563412",
    reversed = "0x6a2c48091e6a2c48",
}

nonzero_integer! {
    Self = NonZeroI128,
    Primitive = signed i128,
    UnsignedPrimitive = u128,
    rot = 16,
    rot_op = "0x13f40000000000000000000000004f76",
    rot_result = "0x4f7613f4",
    swap_op = "0x12345678901234567890123456789012",
    swapped = "0x12907856341290785634129078563412",
    reversed = "0x48091e6a2c48091e6a2c48091e6a2c48",
}

#[cfg(target_pointer_width = "16")]
nonzero_integer! {
    Self = NonZeroIsize,
    Primitive = signed isize,
    UnsignedPrimitive = usize,
    rot = 4,
    rot_op = "-0x5ffd",
    rot_result = "0x3a",
    swap_op = "0x1234",
    swapped = "0x3412",
    reversed = "0x2c48",
}

#[cfg(target_pointer_width = "32")]
nonzero_integer! {
    Self = NonZeroIsize,
    Primitive = signed isize,
    UnsignedPrimitive = usize,
    rot = 8,
    rot_op = "0x10000b3",
    rot_result = "0xb301",
    swap_op = "0x12345678",
    swapped = "0x78563412",
    reversed = "0x1e6a2c48",
}

#[cfg(target_pointer_width = "64")]
nonzero_integer! {
    Self = NonZeroIsize,
    Primitive = signed isize,
    UnsignedPrimitive = usize,
    rot = 12,
    rot_op = "0xaa00000000006e1",
    rot_result = "0x6e10aa",
    swap_op = "0x1234567890123456",
    swapped = "0x5634129078563412",
    reversed = "0x6a2c48091e6a2c48",
}
