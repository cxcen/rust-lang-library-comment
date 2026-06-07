//! 数组基本类型的辅助 API。
//!
//! *[另见数组基本类型](array)。*

#![stable(feature = "core_array", since = "1.35.0")]

use crate::borrow::{Borrow, BorrowMut};
use crate::clone::TrivialClone;
use crate::cmp::Ordering;
use crate::convert::Infallible;
use crate::error::Error;
use crate::hash::{self, Hash};
use crate::intrinsics::transmute_unchecked;
use crate::iter::{UncheckedIterator, repeat_n};
use crate::marker::Destruct;
use crate::mem::{self, ManuallyDrop, MaybeUninit};
use crate::ops::{
    ChangeOutputType, ControlFlow, FromResidual, Index, IndexMut, NeverShortCircuit, Residual, Try,
};
use crate::ptr::{null, null_mut};
use crate::slice::{Iter, IterMut};
use crate::{fmt, ptr};

mod ascii;
mod drain;
mod equality;
mod iter;

#[stable(feature = "array_value_iter", since = "1.51.0")]
pub use iter::IntoIter;

/// 通过反复克隆一个值创建 `[T; N]` 类型的数组。
///
/// 它与 `[val; N]` 类似,但也适用于未实现 [`Copy`] 的类型。
///
/// 传入的值会作为结果数组的一个元素,其余元素通过克隆 `N - 1` 次填满。若 `N` 为 0,
/// 该值会被直接 drop。
///
/// # 示例
///
/// 创建多个 `String` 副本:
/// ```rust
/// use std::array;
///
/// let string = "Hello there!".to_string();
/// let strings = array::repeat(string);
/// assert_eq!(strings, ["Hello there!", "Hello there!"]);
/// ```
#[inline]
#[must_use = "cloning is often expensive and is not expected to have side effects"]
#[stable(feature = "array_repeat", since = "1.91.0")]
pub fn repeat<T: Clone, const N: usize>(val: T) -> [T; N] {
    from_trusted_iterator(repeat_n(val, N))
}

/// 创建一个数组,每个元素都由 `f` 接收该元素索引后生成,遍历顺序为从前到后。
///
/// 这基本等价于写:
/// ```text
/// [f(0), f(1), f(2), …, f(N - 2), f(N - 1)]
/// ```
/// 它类似于 `(0..i).map(f)`,只是目标是数组而不是迭代器。
///
/// 若 `N == 0`,本函数会生成空数组,且完全不会调用 `f`。
///
/// # 示例
///
/// ```rust
/// // 这里类型推断帮了忙:`from_fn` 知道要生成多少元素,是因为下面数组长度参与了比较:
/// // 只有长度相等的数组才能比较,因此 const 泛型参数 `N` 被推断为 5,
/// // 从而创建含 5 个元素的数组。
///
/// let array = core::array::from_fn(|i| i);
/// // 索引为:         0  1  2  3  4
/// assert_eq!(array, [0, 1, 2, 3, 4]);
///
/// let array2: [usize; 8] = core::array::from_fn(|i| i * 2);
/// // 索引为:          0  1  2  3  4  5   6   7
/// assert_eq!(array2, [0, 2, 4, 6, 8, 10, 12, 14]);
///
/// let bool_arr = core::array::from_fn::<_, 5, _>(|i| i % 2 == 0);
/// // 索引为:            0     1      2     3      4
/// assert_eq!(bool_arr, [true, false, true, false, true]);
/// ```
///
/// 闭包也可以捕获环境。例如,当元素不是 `Copy`、不能直接使用 `[item; N]` 时,
/// 可以创建一个充满克隆值的数组:
/// ```
/// # // 实际上 `array::repeat` 更适合这个例子,但它目前尚未稳定。
/// let my_string = String::from("Hello");
/// let clones: [String; 42] = std::array::from_fn(|_| my_string.clone());
/// assert!(clones.iter().all(|x| *x == my_string));
/// ```
///
/// 数组按索引递增顺序生成,也就是从前往后,因此可以使用带可变状态的闭包:
/// ```
/// let mut state = 1;
/// let a = std::array::from_fn(|_| { let x = state; state *= 2; x });
/// assert_eq!(a, [1, 2, 4, 8, 16, 32]);
/// ```
#[inline]
#[stable(feature = "array_from_fn", since = "1.63.0")]
#[rustc_const_unstable(feature = "const_array", issue = "147606")]
pub const fn from_fn<T: [const] Destruct, const N: usize, F>(f: F) -> [T; N]
where
    F: [const] FnMut(usize) -> T + [const] Destruct,
{
    try_from_fn(NeverShortCircuit::wrap_mut_1(f)).0
}

/// 创建一个 `[T; N]` 数组,其中每个可能失败的元素 `T` 都由 `cb` 调用返回。
/// 与元素创建不会失败的 [`from_fn`] 不同,只要任一元素创建失败,本版本就会返回错误。
///
/// 本函数的返回类型取决于闭包返回类型。若闭包返回 `Result<T, E>`,你会得到
/// `Result<[T; N], E>`。若闭包返回 `Option<T>`,你会得到 `Option<[T; N]>`。
///
/// # 参数
///
/// * `cb`: 回调函数,传入参数是当前数组索引。
///
/// # 示例
///
/// ```rust
/// #![feature(array_try_from_fn)]
///
/// let array: Result<[u8; 5], _> = std::array::try_from_fn(|i| i.try_into());
/// assert_eq!(array, Ok([0, 1, 2, 3, 4]));
///
/// let array: Result<[i8; 200], _> = std::array::try_from_fn(|i| i.try_into());
/// assert!(array.is_err());
///
/// let array: Option<[_; 4]> = std::array::try_from_fn(|i| i.checked_add(100));
/// assert_eq!(array, Some([100, 101, 102, 103]));
///
/// let array: Option<[_; 4]> = std::array::try_from_fn(|i| i.checked_sub(100));
/// assert_eq!(array, None);
/// ```
#[inline]
#[unstable(feature = "array_try_from_fn", issue = "89379")]
#[rustc_const_unstable(feature = "array_try_from_fn", issue = "89379")]
pub const fn try_from_fn<R, const N: usize, F>(cb: F) -> ChangeOutputType<R, [R::Output; N]>
where
    R: [const] Try<Residual: [const] Residual<[R::Output; N]>, Output: [const] Destruct>,
    F: [const] FnMut(usize) -> R + [const] Destruct,
{
    let mut array = [const { MaybeUninit::uninit() }; N];
    match try_from_fn_erased(&mut array, cb) {
        ControlFlow::Break(r) => FromResidual::from_residual(r),
        ControlFlow::Continue(()) => {
            // SAFETY: 数组的所有元素都已填充。
            try { unsafe { MaybeUninit::array_assume_init(array) } }
        }
    }
}

/// 把 `T` 的引用转换为长度为 1 的数组引用(不复制)。
#[stable(feature = "array_from_ref", since = "1.53.0")]
#[rustc_const_stable(feature = "const_array_from_ref_shared", since = "1.63.0")]
pub const fn from_ref<T>(s: &T) -> &[T; 1] {
    // SAFETY: 把 `&T` 转换为 `&[T; 1]` 是健全的;单元素数组与元素引用同址且覆盖同一对象。
    unsafe { &*(s as *const T).cast::<[T; 1]>() }
}

/// 把 `T` 的可变引用转换为长度为 1 的可变数组引用(不复制)。
#[stable(feature = "array_from_ref", since = "1.53.0")]
#[rustc_const_stable(feature = "const_array_from_ref", since = "1.83.0")]
pub const fn from_mut<T>(s: &mut T) -> &mut [T; 1] {
    // SAFETY: 把 `&mut T` 转换为 `&mut [T; 1]` 是健全的;独占借用覆盖同一单个对象。
    unsafe { &mut *(s as *mut T).cast::<[T; 1]>() }
}

/// 从切片转换为数组失败时返回的错误类型。
#[stable(feature = "try_from", since = "1.34.0")]
#[derive(Debug, Copy, Clone)]
pub struct TryFromSliceError(());

#[stable(feature = "core_array", since = "1.35.0")]
impl fmt::Display for TryFromSliceError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "could not convert slice to array".fmt(f)
    }
}

#[stable(feature = "try_from", since = "1.34.0")]
impl Error for TryFromSliceError {}

#[stable(feature = "try_from_slice_error", since = "1.36.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<Infallible> for TryFromSliceError {
    fn from(x: Infallible) -> TryFromSliceError {
        match x {}
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T, const N: usize> const AsRef<[T]> for [T; N] {
    #[inline]
    fn as_ref(&self) -> &[T] {
        &self[..]
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T, const N: usize> const AsMut<[T]> for [T; N] {
    #[inline]
    fn as_mut(&mut self) -> &mut [T] {
        &mut self[..]
    }
}

#[stable(feature = "array_borrow", since = "1.4.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T, const N: usize> const Borrow<[T]> for [T; N] {
    fn borrow(&self) -> &[T] {
        self
    }
}

#[stable(feature = "array_borrow", since = "1.4.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T, const N: usize> const BorrowMut<[T]> for [T; N] {
    fn borrow_mut(&mut self) -> &mut [T] {
        self
    }
}

/// 尝试通过从切片 `&[T]` 复制来创建数组 `[T; N]`。
/// 当 `slice.len() == N` 时成功。
///
/// ```
/// let bytes: [u8; 3] = [1, 0, 2];
///
/// let bytes_head: [u8; 2] = <[u8; 2]>::try_from(&bytes[0..2]).unwrap();
/// assert_eq!(1, u16::from_le_bytes(bytes_head));
///
/// let bytes_tail: [u8; 2] = bytes[1..3].try_into().unwrap();
/// assert_eq!(512, u16::from_le_bytes(bytes_tail));
/// ```
#[stable(feature = "try_from", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T, const N: usize> const TryFrom<&[T]> for [T; N]
where
    T: Copy,
{
    type Error = TryFromSliceError;

    #[inline]
    fn try_from(slice: &[T]) -> Result<[T; N], TryFromSliceError> {
        <&Self>::try_from(slice).copied()
    }
}

/// 尝试通过从可变切片 `&mut [T]` 复制来创建数组 `[T; N]`。
/// 当 `slice.len() == N` 时成功。
///
/// ```
/// let mut bytes: [u8; 3] = [1, 0, 2];
///
/// let bytes_head: [u8; 2] = <[u8; 2]>::try_from(&mut bytes[0..2]).unwrap();
/// assert_eq!(1, u16::from_le_bytes(bytes_head));
///
/// let bytes_tail: [u8; 2] = (&mut bytes[1..3]).try_into().unwrap();
/// assert_eq!(512, u16::from_le_bytes(bytes_tail));
/// ```
#[stable(feature = "try_from_mut_slice_to_array", since = "1.59.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T, const N: usize> const TryFrom<&mut [T]> for [T; N]
where
    T: Copy,
{
    type Error = TryFromSliceError;

    #[inline]
    fn try_from(slice: &mut [T]) -> Result<[T; N], TryFromSliceError> {
        <Self>::try_from(&*slice)
    }
}

/// 尝试从切片引用 `&[T]` 创建数组引用 `&[T; N]`。当 `slice.len() == N` 时成功。
///
/// ```
/// let bytes: [u8; 3] = [1, 0, 2];
///
/// let bytes_head: &[u8; 2] = <&[u8; 2]>::try_from(&bytes[0..2]).unwrap();
/// assert_eq!(1, u16::from_le_bytes(*bytes_head));
///
/// let bytes_tail: &[u8; 2] = bytes[1..3].try_into().unwrap();
/// assert_eq!(512, u16::from_le_bytes(*bytes_tail));
/// ```
#[stable(feature = "try_from", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<'a, T, const N: usize> const TryFrom<&'a [T]> for &'a [T; N] {
    type Error = TryFromSliceError;

    #[inline]
    fn try_from(slice: &'a [T]) -> Result<&'a [T; N], TryFromSliceError> {
        slice.as_array().ok_or(TryFromSliceError(()))
    }
}

/// 尝试从可变切片引用 `&mut [T]` 创建可变数组引用 `&mut [T; N]`。
/// 当 `slice.len() == N` 时成功。
///
/// ```
/// let mut bytes: [u8; 3] = [1, 0, 2];
///
/// let bytes_head: &mut [u8; 2] = <&mut [u8; 2]>::try_from(&mut bytes[0..2]).unwrap();
/// assert_eq!(1, u16::from_le_bytes(*bytes_head));
///
/// let bytes_tail: &mut [u8; 2] = (&mut bytes[1..3]).try_into().unwrap();
/// assert_eq!(512, u16::from_le_bytes(*bytes_tail));
/// ```
#[stable(feature = "try_from", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<'a, T, const N: usize> const TryFrom<&'a mut [T]> for &'a mut [T; N] {
    type Error = TryFromSliceError;

    #[inline]
    fn try_from(slice: &'a mut [T]) -> Result<&'a mut [T; N], TryFromSliceError> {
        slice.as_mut_array().ok_or(TryFromSliceError(()))
    }
}

/// 数组的 hash 与对应切片的 hash 相同,这是 `Borrow` 实现所要求的契约。
///
/// ```
/// use std::hash::BuildHasher;
///
/// let b = std::hash::RandomState::new();
/// let a: [u8; 3] = [0xa8, 0x3c, 0x09];
/// let s: &[u8] = &[0xa8, 0x3c, 0x09];
/// assert_eq!(b.hash_one(a), b.hash_one(s));
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
impl<T: Hash, const N: usize> Hash for [T; N] {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        Hash::hash(&self[..], state)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: fmt::Debug, const N: usize> fmt::Debug for [T; N] {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&&self[..], f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T, const N: usize> IntoIterator for &'a [T; N] {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T, const N: usize> IntoIterator for &'a mut [T; N] {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}

#[stable(feature = "index_trait_on_arrays", since = "1.50.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
impl<T, I, const N: usize> const Index<I> for [T; N]
where
    [T]: [const] Index<I>,
{
    type Output = <[T] as Index<I>>::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        Index::index(self as &[T], index)
    }
}

#[stable(feature = "index_trait_on_arrays", since = "1.50.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
impl<T, I, const N: usize> const IndexMut<I> for [T; N]
where
    [T]: [const] IndexMut<I>,
{
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(self as &mut [T], index)
    }
}

/// 按[字典序](Ord#lexicographical-comparison)实现数组比较。
#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PartialOrd, const N: usize> PartialOrd for [T; N] {
    #[inline]
    fn partial_cmp(&self, other: &[T; N]) -> Option<Ordering> {
        PartialOrd::partial_cmp(&&self[..], &&other[..])
    }
    #[inline]
    fn lt(&self, other: &[T; N]) -> bool {
        PartialOrd::lt(&&self[..], &&other[..])
    }
    #[inline]
    fn le(&self, other: &[T; N]) -> bool {
        PartialOrd::le(&&self[..], &&other[..])
    }
    #[inline]
    fn ge(&self, other: &[T; N]) -> bool {
        PartialOrd::ge(&&self[..], &&other[..])
    }
    #[inline]
    fn gt(&self, other: &[T; N]) -> bool {
        PartialOrd::gt(&&self[..], &&other[..])
    }
}

/// 按[字典序](Ord#lexicographical-comparison)实现数组比较。
#[stable(feature = "rust1", since = "1.0.0")]
impl<T: Ord, const N: usize> Ord for [T; N] {
    #[inline]
    fn cmp(&self, other: &[T; N]) -> Ordering {
        Ord::cmp(&&self[..], &&other[..])
    }
}

#[stable(feature = "copy_clone_array_lib", since = "1.58.0")]
impl<T: Copy, const N: usize> Copy for [T; N] {}

#[stable(feature = "copy_clone_array_lib", since = "1.58.0")]
impl<T: Clone, const N: usize> Clone for [T; N] {
    #[inline]
    fn clone(&self) -> Self {
        SpecArrayClone::clone(self)
    }

    #[inline]
    fn clone_from(&mut self, other: &Self) {
        self.clone_from_slice(other);
    }
}

#[doc(hidden)]
#[unstable(feature = "trivial_clone", issue = "none")]
unsafe impl<T: TrivialClone, const N: usize> TrivialClone for [T; N] {}

trait SpecArrayClone: Clone {
    fn clone<const N: usize>(array: &[Self; N]) -> [Self; N];
}

impl<T: Clone> SpecArrayClone for T {
    #[inline]
    default fn clone<const N: usize>(array: &[T; N]) -> [T; N] {
        from_trusted_iterator(array.iter().cloned())
    }
}

impl<T: TrivialClone> SpecArrayClone for T {
    #[inline]
    fn clone<const N: usize>(array: &[T; N]) -> [T; N] {
        // SAFETY: `TrivialClone` 意味着这等价于对每个元素调用 `Clone`。
        unsafe { ptr::read(array) }
    }
}

// Default impl 不能用 const generics 完成,因为 `[T; 0]` 不要求实现 Default,
// 而当前还不支持按不同数字提供不同 impl block。
//
// 事实证明,改善 `[T; 0]` 的情况很困难。过去的尝试和 crater 运行见这些 issue:
// - https://github.com/rust-lang/rust/issues/61415
// - https://github.com/rust-lang/rust/pull/145457

macro_rules! array_impl_default {
    {$n:expr, $t:ident $($ts:ident)*} => {
        #[stable(since = "1.4.0", feature = "array_default")]
        impl<T> Default for [T; $n] where T: Default {
            fn default() -> [T; $n] {
                [$t::default(), $($ts::default()),*]
            }
        }
        array_impl_default!{($n - 1), $($ts)*}
    };
    {$n:expr,} => {
        #[stable(since = "1.4.0", feature = "array_default")]
        impl<T> Default for [T; $n] {
            fn default() -> [T; $n] { [] }
        }
    };
}

array_impl_default! {32, T T T T T T T T T T T T T T T T T T T T T T T T T T T T T T T T}

impl<T, const N: usize> [T; N] {
    /// 返回一个与 `self` 大小相同的新数组,其中每个元素都按顺序应用函数 `f`。
    ///
    /// 如果不一定需要新的定长数组,请考虑改用 [`Iterator::map`]。
    ///
    ///
    /// # 性能与栈使用说明
    ///
    /// 遗憾的是,本方法目前并不总能被优化到理想状态。这主要影响大数组;小数组上的映射通常
    /// 优化得很好。还需注意,在 debug 模式(即没有优化)下,本方法可能使用大量栈空间
    /// (数倍于数组大小,甚至更多)。
    ///
    /// 因此,在性能关键代码中,应尽量避免对大数组使用本方法,或检查生成代码。也应尽量避免
    /// 链式 map(例如 `arr.map(...).map(...)`)。
    ///
    /// 很多情况下,可以对数组调用 `.iter()` 或 `.into_iter()`,再使用 [`Iterator::map`]。
    /// 只有当你确实需要一个与结果同大小的新数组时,才需要 `[T; N]::map`。
    /// Rust 的惰性迭代器通常能获得很好的优化。
    ///
    ///
    /// # 示例
    ///
    /// ```
    /// let x = [1, 2, 3];
    /// let y = x.map(|v| v + 1);
    /// assert_eq!(y, [2, 3, 4]);
    ///
    /// let x = [1, 2, 3];
    /// let mut temp = 0;
    /// let y = x.map(|v| { temp += 1; v * temp });
    /// assert_eq!(y, [1, 4, 9]);
    ///
    /// let x = ["Ferris", "Bueller's", "Day", "Off"];
    /// let y = x.map(|v| v.len());
    /// assert_eq!(y, [6, 9, 3, 3]);
    /// ```
    #[must_use]
    #[stable(feature = "array_map", since = "1.55.0")]
    #[rustc_const_unstable(feature = "const_array", issue = "147606")]
    pub const fn map<F, U>(self, f: F) -> [U; N]
    where
        F: [const] FnMut(T) -> U + [const] Destruct,
        U: [const] Destruct,
        T: [const] Destruct,
    {
        self.try_map(NeverShortCircuit::wrap_mut_1(f)).0
    }

    /// 按顺序对数组 `self` 的每个元素应用可能失败的函数 `f`,返回与 `self` 同大小的数组,
    /// 或返回遇到的第一个错误。
    ///
    /// 本函数的返回类型取决于闭包返回类型。若闭包返回 `Result<T, E>`,你会得到
    /// `Result<[T; N], E>`。若闭包返回 `Option<T>`,你会得到 `Option<[T; N]>`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(array_try_map)]
    ///
    /// let a = ["1", "2", "3"];
    /// let b = a.try_map(|v| v.parse::<u32>()).unwrap().map(|v| v + 1);
    /// assert_eq!(b, [2, 3, 4]);
    ///
    /// let a = ["1", "2a", "3"];
    /// let b = a.try_map(|v| v.parse::<u32>());
    /// assert!(b.is_err());
    ///
    /// use std::num::NonZero;
    ///
    /// let z = [1, 2, 0, 3, 4];
    /// assert_eq!(z.try_map(NonZero::new), None);
    ///
    /// let a = [1, 2, 3];
    /// let b = a.try_map(NonZero::new);
    /// let c = b.map(|x| x.map(NonZero::get));
    /// assert_eq!(c, Some(a));
    /// ```
    #[unstable(feature = "array_try_map", issue = "79711")]
    #[rustc_const_unstable(feature = "array_try_map", issue = "79711")]
    pub const fn try_map<R>(
        self,
        mut f: impl [const] FnMut(T) -> R + [const] Destruct,
    ) -> ChangeOutputType<R, [R::Output; N]>
    where
        R: [const] Try<Residual: [const] Residual<[R::Output; N]>, Output: [const] Destruct>,
        T: [const] Destruct,
    {
        let mut me = ManuallyDrop::new(self);
        // SAFETY: `try_from_fn` 会调用 `f` 恰好 N 次;Drain 的调用次数契约由此满足。
        let mut f = unsafe { drain::Drain::new(&mut me, &mut f) };
        try_from_fn(&mut f)
    }

    /// 返回包含整个数组的切片。等价于 `&s[..]`。
    #[stable(feature = "array_as_slice", since = "1.57.0")]
    #[rustc_const_stable(feature = "array_as_slice", since = "1.57.0")]
    pub const fn as_slice(&self) -> &[T] {
        self
    }

    /// 返回包含整个数组的可变切片。等价于 `&mut s[..]`。
    #[stable(feature = "array_as_slice", since = "1.57.0")]
    #[rustc_const_stable(feature = "const_array_as_mut_slice", since = "1.89.0")]
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        self
    }

    /// 借用每个元素,并返回一个与 `self` 大小相同的引用数组。
    ///
    ///
    /// # 示例
    ///
    /// ```
    /// let floats = [3.1, 2.7, -1.0];
    /// let float_refs: [&f64; 3] = floats.each_ref();
    /// assert_eq!(float_refs, [&3.1, &2.7, &-1.0]);
    /// ```
    ///
    /// 本方法与 [`map`](#method.map) 等方法组合时特别有用。这样一来,即使元素不是
    /// [`Copy`],也能避免移动原数组。
    ///
    /// ```
    /// let strings = ["Ferris".to_string(), "♥".to_string(), "Rust".to_string()];
    /// let is_ascii = strings.each_ref().map(|s| s.is_ascii());
    /// assert_eq!(is_ascii, [true, false, true]);
    ///
    /// // 仍可访问原数组:它没有被移动。
    /// assert_eq!(strings.len(), 3);
    /// ```
    #[stable(feature = "array_methods", since = "1.77.0")]
    #[rustc_const_stable(feature = "const_array_each_ref", since = "1.91.0")]
    pub const fn each_ref(&self) -> [&T; N] {
        let mut buf = [null::<T>(); N];

        // FIXME(const_trait_impl): 这里本想直接使用迭代器(与原始实现一样),
        // 但常量表达式中尚不允许这样做。
        let mut i = 0;
        while i < N {
            buf[i] = &raw const self[i];

            i += 1;
        }

        // SAFETY: `*const T` 与 `&T` 具有相同布局,且每个指针都已初始化为有效引用。
        unsafe { transmute_unchecked(buf) }
    }

    /// 可变借用每个元素,并返回一个与 `self` 大小相同的可变引用数组。
    ///
    ///
    /// # 示例
    ///
    /// ```
    ///
    /// let mut floats = [3.1, 2.7, -1.0];
    /// let float_refs: [&mut f64; 3] = floats.each_mut();
    /// *float_refs[0] = 0.0;
    /// assert_eq!(float_refs, [&mut 0.0, &mut 2.7, &mut -1.0]);
    /// assert_eq!(floats, [0.0, 2.7, -1.0]);
    /// ```
    #[stable(feature = "array_methods", since = "1.77.0")]
    #[rustc_const_stable(feature = "const_array_each_ref", since = "1.91.0")]
    pub const fn each_mut(&mut self) -> [&mut T; N] {
        let mut buf = [null_mut::<T>(); N];

        // FIXME(const_trait_impl): 这里本想直接使用迭代器(与原始实现一样),
        // 但常量表达式中尚不允许这样做。
        let mut i = 0;
        while i < N {
            buf[i] = &raw mut self[i];

            i += 1;
        }

        // SAFETY: `*mut T` 与 `&mut T` 具有相同布局,且每个指针都已初始化为有效可变引用。
        unsafe { transmute_unchecked(buf) }
    }

    /// 在索引处把一个数组引用拆成两部分。
    ///
    /// 第一部分包含 `[0, M)` 中的所有索引(不含索引 `M` 本身),第二部分包含
    /// `[M, N)` 中的所有索引(不含索引 `N` 本身)。
    ///
    /// # Panics
    ///
    /// 当 `M > N` 时 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(split_array)]
    ///
    /// let v = [1, 2, 3, 4, 5, 6];
    ///
    /// {
    ///    let (left, right) = v.split_array_ref::<0>();
    ///    assert_eq!(left, &[]);
    ///    assert_eq!(right, &[1, 2, 3, 4, 5, 6]);
    /// }
    ///
    /// {
    ///     let (left, right) = v.split_array_ref::<2>();
    ///     assert_eq!(left, &[1, 2]);
    ///     assert_eq!(right, &[3, 4, 5, 6]);
    /// }
    ///
    /// {
    ///     let (left, right) = v.split_array_ref::<6>();
    ///     assert_eq!(left, &[1, 2, 3, 4, 5, 6]);
    ///     assert_eq!(right, &[]);
    /// }
    /// ```
    #[unstable(
        feature = "split_array",
        reason = "return type should have array as 2nd element",
        issue = "90091"
    )]
    #[inline]
    pub fn split_array_ref<const M: usize>(&self) -> (&[T; M], &[T]) {
        self.split_first_chunk::<M>().unwrap()
    }

    /// 在索引处把一个可变数组引用拆成两部分。
    ///
    /// 第一部分包含 `[0, M)` 中的所有索引(不含索引 `M` 本身),第二部分包含
    /// `[M, N)` 中的所有索引(不含索引 `N` 本身)。
    ///
    /// # Panics
    ///
    /// 当 `M > N` 时 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(split_array)]
    ///
    /// let mut v = [1, 0, 3, 0, 5, 6];
    /// let (left, right) = v.split_array_mut::<2>();
    /// assert_eq!(left, &mut [1, 0][..]);
    /// assert_eq!(right, &mut [3, 0, 5, 6]);
    /// left[1] = 2;
    /// right[1] = 4;
    /// assert_eq!(v, [1, 2, 3, 4, 5, 6]);
    /// ```
    #[unstable(
        feature = "split_array",
        reason = "return type should have array as 2nd element",
        issue = "90091"
    )]
    #[inline]
    pub fn split_array_mut<const M: usize>(&mut self) -> (&mut [T; M], &mut [T]) {
        self.split_first_chunk_mut::<M>().unwrap()
    }

    /// 从尾部计数,在索引处把一个数组引用拆成两部分。
    ///
    /// 第一部分包含 `[0, N - M)` 中的所有索引(不含索引 `N - M` 本身),第二部分包含
    /// `[N - M, N)` 中的所有索引(不含索引 `N` 本身)。
    ///
    /// # Panics
    ///
    /// 当 `M > N` 时 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(split_array)]
    ///
    /// let v = [1, 2, 3, 4, 5, 6];
    ///
    /// {
    ///    let (left, right) = v.rsplit_array_ref::<0>();
    ///    assert_eq!(left, &[1, 2, 3, 4, 5, 6]);
    ///    assert_eq!(right, &[]);
    /// }
    ///
    /// {
    ///     let (left, right) = v.rsplit_array_ref::<2>();
    ///     assert_eq!(left, &[1, 2, 3, 4]);
    ///     assert_eq!(right, &[5, 6]);
    /// }
    ///
    /// {
    ///     let (left, right) = v.rsplit_array_ref::<6>();
    ///     assert_eq!(left, &[]);
    ///     assert_eq!(right, &[1, 2, 3, 4, 5, 6]);
    /// }
    /// ```
    #[unstable(
        feature = "split_array",
        reason = "return type should have array as 2nd element",
        issue = "90091"
    )]
    #[inline]
    pub fn rsplit_array_ref<const M: usize>(&self) -> (&[T], &[T; M]) {
        self.split_last_chunk::<M>().unwrap()
    }

    /// 从尾部计数,在索引处把一个可变数组引用拆成两部分。
    ///
    /// 第一部分包含 `[0, N - M)` 中的所有索引(不含索引 `N - M` 本身),第二部分包含
    /// `[N - M, N)` 中的所有索引(不含索引 `N` 本身)。
    ///
    /// # Panics
    ///
    /// 当 `M > N` 时 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(split_array)]
    ///
    /// let mut v = [1, 0, 3, 0, 5, 6];
    /// let (left, right) = v.rsplit_array_mut::<4>();
    /// assert_eq!(left, &mut [1, 0]);
    /// assert_eq!(right, &mut [3, 0, 5, 6][..]);
    /// left[1] = 2;
    /// right[1] = 4;
    /// assert_eq!(v, [1, 2, 3, 4, 5, 6]);
    /// ```
    #[unstable(
        feature = "split_array",
        reason = "return type should have array as 2nd element",
        issue = "90091"
    )]
    #[inline]
    pub fn rsplit_array_mut<const M: usize>(&mut self) -> (&mut [T], &mut [T; M]) {
        self.split_last_chunk_mut::<M>().unwrap()
    }
}

/// 使用 `iter` 的前 `N` 个元素填充数组。
///
/// # Panics
///
/// 若迭代器实际没有足够元素。
///
/// 不过,依赖 `TrustedLen` 后,我们可以预先完成这个检查(这里很容易被优化掉),
/// 因而不会影响填充数组的循环。
#[inline]
fn from_trusted_iterator<T, const N: usize>(iter: impl UncheckedIterator<Item = T>) -> [T; N] {
    try_from_trusted_iterator(iter.map(NeverShortCircuit)).0
}

#[inline]
fn try_from_trusted_iterator<T, R, const N: usize>(
    iter: impl UncheckedIterator<Item = R>,
) -> ChangeOutputType<R, [T; N]>
where
    R: Try<Output = T>,
    R::Residual: Residual<[T; N]>,
{
    assert!(iter.size_hint().0 >= N);
    fn next<T>(mut iter: impl UncheckedIterator<Item = T>) -> impl FnMut(usize) -> T {
        move |_| {
            // SAFETY: 我们知道 `from_fn` 最多调用这里 N 次,且已经检查过至少有这么多元素。
            unsafe { iter.next_unchecked() }
        }
    }

    try_from_fn(next(iter))
}

/// [`try_from_fn`] 的切片版本,通过传入切片避免为每种数组长度单独单态化。
///
/// 它接收 generator 而不是迭代器,这样在*类型层面*永远不需要担心元素耗尽。
/// 与不会失败的 `Try` 类型结合时,这意味着循环很容易规范化,从而获得良好优化。
///
/// 从技术上说,*可以*把本函数与 [`iter_next_chunk_erased`] 合并成一个同时处理两种需求的函数。
/// 但上一次这样做时,“源元素是否足够?”的检查没能优化掉,导致 codegen 变差。
/// 因此若再次尝试,务必关注 codegen 测试中的变化。
#[inline]
#[rustc_const_unstable(feature = "array_try_from_fn", issue = "89379")]
const fn try_from_fn_erased<R: [const] Try<Output: [const] Destruct>>(
    buffer: &mut [MaybeUninit<R::Output>],
    mut generator: impl [const] FnMut(usize) -> R + [const] Destruct,
) -> ControlFlow<R::Residual> {
    let mut guard = Guard { array_mut: buffer, initialized: 0 };

    while guard.initialized < guard.array_mut.len() {
        let item = generator(guard.initialized).branch()?;

        // SAFETY: 循环条件保证仍有空间写入该元素。
        unsafe { guard.push_unchecked(item) };
    }

    mem::forget(guard);
    ControlFlow::Continue(())
}

/// 数组增量初始化过程的 panic guard。
///
/// 数组初始化完成后,用 `mem::forget` 解除 guard。
///
/// # 安全性(Safety）
///
/// 对本结构的所有写访问都是 unsafe 的,必须始终维护正确的 `initialized` 元素计数。
///
/// 为减少间接层,字段仍是 pub;但调用方至少应使用 `push_unchecked`,以明确这里正在执行
/// unsafe 初始化协议。
struct Guard<'a, T> {
    /// 正在初始化的数组。
    pub array_mut: &'a mut [MaybeUninit<T>],
    /// 目前已经初始化的元素数量。
    pub initialized: usize,
}

impl<T> Guard<'_, T> {
    /// 向数组添加一个元素,并更新已初始化元素计数器。
    ///
    /// # 安全性(Safety）
    ///
    /// 初始化元素数量不得超过数组长度 N。
    #[inline]
    #[rustc_const_unstable(feature = "array_try_from_fn", issue = "89379")]
    pub(crate) const unsafe fn push_unchecked(&mut self, item: T) {
        // SAFETY: 若调用前 `initialized` 正确,且调用方没有超过 N 次调用本方法,
        // 则写入一定在边界内,且每个槽位不会被初始化超过一次。
        unsafe {
            self.array_mut.get_unchecked_mut(self.initialized).write(item);
            self.initialized = self.initialized.unchecked_add(1);
        }
    }
}

#[rustc_const_unstable(feature = "array_try_from_fn", issue = "89379")]
impl<T: [const] Destruct> const Drop for Guard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        debug_assert!(self.initialized <= self.array_mut.len());
        // SAFETY: 该切片只包含已经初始化的对象。
        unsafe {
            self.array_mut.get_unchecked_mut(..self.initialized).assume_init_drop();
        }
    }
}

/// 从 `iter` 拉取 `N` 个元素并以数组形式返回。若迭代器产出的元素少于 `N`,
/// 返回 `Err`,其中包含一个遍历已产出元素的迭代器。
///
/// 由于迭代器以可变引用传入,且本函数最多调用 `next` N 次,调用后仍可继续使用该迭代器
/// 取得剩余元素。
///
/// 若 `iter.next()` panic,迭代器已经产出的所有元素都会被 drop。
///
/// 供 [`Iterator::next_chunk`] 使用。
#[inline]
pub(crate) fn iter_next_chunk<T, const N: usize>(
    iter: &mut impl Iterator<Item = T>,
) -> Result<[T; N], IntoIter<T, N>> {
    let mut array = [const { MaybeUninit::uninit() }; N];
    let r = iter_next_chunk_erased(&mut array, iter);
    match r {
        Ok(()) => {
            // SAFETY: `array` 的所有元素都已填充。
            Ok(unsafe { MaybeUninit::array_assume_init(array) })
        }
        Err(initialized) => {
            // SAFETY: 只有前 `initialized` 个元素已经填充。
            Err(unsafe { IntoIter::new_unchecked(array, 0..initialized) })
        }
    }
}

/// [`iter_next_chunk`] 的切片版本,通过传入切片避免为每种数组长度单独单态化。
///
/// 遗憾的是,该循环有两个退出条件:缓冲区填满,或迭代器耗尽元素。这会让它倾向于优化不佳。
#[inline]
fn iter_next_chunk_erased<T>(
    buffer: &mut [MaybeUninit<T>],
    iter: &mut impl Iterator<Item = T>,
) -> Result<(), usize> {
    // 若 `Iterator::next` panic,该 guard 会 drop 已经初始化的元素。
    let mut guard = Guard { array_mut: buffer, initialized: 0 };
    while guard.initialized < guard.array_mut.len() {
        let Some(item) = iter.next() else {
            // 不同于 `try_from_fn_erased`,这里需要保留部分结果,
            // 因此要解除 guard,而不是使用 `?`。
            let initialized = guard.initialized;
            mem::forget(guard);
            return Err(initialized);
        };

        // SAFETY: 循环条件保证仍有空间写入该元素。
        unsafe { guard.push_unchecked(item) };
    }

    mem::forget(guard);
    Ok(())
}
