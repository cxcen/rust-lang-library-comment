use crate::num::NonZero;

/// 这样一些类型:对它们而言,`==` 与 `!=` 等价于直接比较其底层字节。
///
/// 重要的是,这意味着它不包含浮点类型,因为浮点数有不同的字节表示(例如
/// `-0` 和 `+0`),而这些表示在比较时被视为相等。由于字节数组是 `Eq`,
/// 这隐含着这些类型很可能也是 `Eq`,不过这在技术上并非使用本 trait 的
/// 必要条件。
///
/// `Rhs` *实际上* 总是 `Self`,但把它单独作为一个参数很重要,这是为了在
/// 消费本 trait 时避免出现 `specializing impl repeats parameter` 错误。
///
/// # 安全性(Safety)
///
/// - `Self` 与 `Rhs` 没有填充(padding)。
/// - `Self` 与 `Rhs` 具有相同的内存布局(大小与对齐)。
/// - `Self` 与 `Rhs` 都不带 provenance,因此整数比较是正确的。
/// - `<Self as PartialEq<Rhs>>::{eq,ne}` 等价于直接比较字节。
#[rustc_specialization_trait]
pub(crate) const unsafe trait BytewiseEq<Rhs = Self>:
    [const] PartialEq<Rhs> + Sized
{
}

macro_rules! is_bytewise_comparable {
    ($($t:ty),+ $(,)?) => {$(
        unsafe impl const BytewiseEq for $t {}
    )+};
}

// SAFETY:所有普通整数类型都没有填充,而且不是指针。
is_bytewise_comparable!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

// SAFETY:这些类型有 *niche*,但没有 *填充* 也没有 *provenance*,
// 所以我们可以直接比较它们。
is_bytewise_comparable!(bool, char, super::Ordering);

// SAFETY:同理,`NonZero` 类型有一个 niche,但没有 undef、也没有指针,
// 而且它们的比较行为与其底层数值类型一致。
is_bytewise_comparable!(
    NonZero<u8>,
    NonZero<u16>,
    NonZero<u32>,
    NonZero<u64>,
    NonZero<u128>,
    NonZero<usize>,
    NonZero<i8>,
    NonZero<i16>,
    NonZero<i32>,
    NonZero<i64>,
    NonZero<i128>,
    NonZero<isize>,
);

// SAFETY:`NonZero` 类型有被保证的“null”优化,因此把它放在 `Option` 里时,
// 按位进行相等比较也是安全的。
// `Option` 的 `PartialOrd` 的定义方式意味着,这对有符号类型上的 `<` 或 `>`
// 是行不通的,但既然我们只做 `==`,那就没问题。
is_bytewise_comparable!(
    Option<NonZero<u8>>,
    Option<NonZero<u16>>,
    Option<NonZero<u32>>,
    Option<NonZero<u64>>,
    Option<NonZero<u128>>,
    Option<NonZero<usize>>,
    Option<NonZero<i8>>,
    Option<NonZero<i16>>,
    Option<NonZero<i32>>,
    Option<NonZero<i64>>,
    Option<NonZero<i128>>,
    Option<NonZero<isize>>,
);

macro_rules! is_bytewise_comparable_array_length {
    ($($n:literal),+ $(,)?) => {$(
        // SAFETY:数组的各元素之间没有填充,所以如果元素是 `BytewiseEq`,
        // 那么整个数组也可以是。
        unsafe impl<T: BytewiseEq<U>, U> BytewiseEq<[U; $n]> for [T; $n] {}
    )+};
}

// 令人沮丧的是,这无法做成 const 泛型,因为那样会得到
//    error: specializing impl repeats parameter `N`
// 所以只好针对几个貌似常见的长度逐个实现。
is_bytewise_comparable_array_length!(0, 1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64);
