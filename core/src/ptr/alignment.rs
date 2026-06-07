#![allow(clippy::enum_clike_unportable_variant)]

use crate::num::NonZero;
use crate::ub_checks::assert_unsafe_precondition;
use crate::{cmp, fmt, hash, mem, num};

/// 一个存储 `usize` 的类型,该 `usize` 是 2 的幂(power of two),因而表示 Rust
/// 抽象机(abstract machine)中一个可能的对齐(alignment)值。
///
/// 注意:特别大的对齐值虽然可以由此类型表示,但很可能不被实际的分配器(allocator)
/// 和链接器(linker)所支持。
#[unstable(feature = "ptr_alignment_type", issue = "102070")]
#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct Alignment(AlignmentEnum);

// Alignment 是 `repr(usize)`,只不过是通过额外的间接手段实现的。
const _: () = assert!(size_of::<Alignment>() == size_of::<usize>());
const _: () = assert!(align_of::<Alignment>() == align_of::<usize>());

fn _alignment_can_be_structurally_matched(a: Alignment) -> bool {
    matches!(a, Alignment::MIN)
}

impl Alignment {
    /// 可能的最小对齐值,即 1。
    ///
    /// 所有地址至少都对齐到这个程度(任何地址都是 1 字节对齐的)。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ptr_alignment_type)]
    /// use std::ptr::Alignment;
    ///
    /// assert_eq!(Alignment::MIN.as_usize(), 1);
    /// ```
    #[unstable(feature = "ptr_alignment_type", issue = "102070")]
    pub const MIN: Self = Self(AlignmentEnum::_Align1Shl0);

    /// 返回某个类型的对齐值。
    ///
    /// 它给出的数值与 [`align_of`] 相同,只不过包装在 `Alignment` 中而非 `usize`。
    #[unstable(feature = "ptr_alignment_type", issue = "102070")]
    #[inline]
    #[must_use]
    pub const fn of<T>() -> Self {
        // 这里实际上不可能 panic,因为类型对齐值永远是 2 的幂。
        const { Alignment::new(align_of::<T>()).unwrap() }
    }

    /// 从一个 `usize` 创建 `Alignment`;若它不是 2 的幂,则返回 `None`。
    ///
    /// 注意:`0` 既不是 2 的幂,也不是一个有效的对齐值。
    #[unstable(feature = "ptr_alignment_type", issue = "102070")]
    #[inline]
    pub const fn new(align: usize) -> Option<Self> {
        if align.is_power_of_two() {
            // SAFETY: 刚刚检查过它只设置了一个比特位(即是 2 的幂)。
            Some(unsafe { Self::new_unchecked(align) })
        } else {
            None
        }
    }

    /// 从一个 2 的幂的 `usize` 创建 `Alignment`。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须保证:`align` 是 2 的幂。
    ///
    /// 等价地说,它必须是某个 `exp`(取值范围 `0..usize::BITS`)对应的 `1 << exp`。
    /// 它**绝不能**为零。
    #[unstable(feature = "ptr_alignment_type", issue = "102070")]
    #[inline]
    #[track_caller]
    pub const unsafe fn new_unchecked(align: usize) -> Self {
        assert_unsafe_precondition!(
            check_language_ub,
            "Alignment::new_unchecked requires a power of two",
            (align: usize = align) => align.is_power_of_two()
        );

        // SAFETY: 根据前置条件,它必定是 2 的幂,而我们的枚举变体涵盖了所有可能的
        // 2 的幂取值。
        unsafe { mem::transmute::<usize, Alignment>(align) }
    }

    /// 以 [`usize`] 形式返回该对齐值。
    #[unstable(feature = "ptr_alignment_type", issue = "102070")]
    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// 以 <code>[NonZero]<[usize]></code> 形式返回该对齐值。
    #[unstable(feature = "ptr_alignment_type", issue = "102070")]
    #[inline]
    pub const fn as_nonzero(self) -> NonZero<usize> {
        // 这里直接 transmute,以避开 `NonZero::new_unchecked` 中的 UbCheck 检查:
        // 反正用户也没有办法触发该检查——除非该类型的有效性不变量在更早的地方就已被
        // 破坏——而在这样一个本应简单的方法里发出该检查,对编译时间不利。

        // SAFETY: 所有判别值(discriminant)都非零。
        unsafe { mem::transmute::<Alignment, NonZero<usize>>(self) }
    }

    /// 返回该对齐值以 2 为底的对数(base-2 logarithm)。
    ///
    /// 由于 `self` 表示的是 2 的幂,该结果永远是精确的。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ptr_alignment_type)]
    /// use std::ptr::Alignment;
    ///
    /// assert_eq!(Alignment::of::<u8>().log2(), 0);
    /// assert_eq!(Alignment::new(1024).unwrap().log2(), 10);
    /// ```
    #[unstable(feature = "ptr_alignment_type", issue = "102070")]
    #[inline]
    pub const fn log2(self) -> u32 {
        self.as_nonzero().trailing_zeros()
    }

    /// 返回一个可用于匹配此对齐值的位掩码(bit mask)。
    ///
    /// 它等价于 `!(self.as_usize() - 1)`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ptr_alignment_type)]
    /// #![feature(ptr_mask)]
    /// use std::ptr::{Alignment, NonNull};
    ///
    /// #[repr(align(1))] struct Align1(u8);
    /// #[repr(align(2))] struct Align2(u16);
    /// #[repr(align(4))] struct Align4(u32);
    /// let one = <NonNull<Align1>>::dangling().as_ptr();
    /// let two = <NonNull<Align2>>::dangling().as_ptr();
    /// let four = <NonNull<Align4>>::dangling().as_ptr();
    ///
    /// assert_eq!(four.mask(Alignment::of::<Align1>().mask()), four);
    /// assert_eq!(four.mask(Alignment::of::<Align2>().mask()), four);
    /// assert_eq!(four.mask(Alignment::of::<Align4>().mask()), four);
    /// assert_ne!(one.mask(Alignment::of::<Align4>().mask()), one);
    /// ```
    #[unstable(feature = "ptr_alignment_type", issue = "102070")]
    #[inline]
    pub const fn mask(self) -> usize {
        // SAFETY: 对齐值永远非零,因此减一不会发生下溢(overflow)。
        !(unsafe { self.as_usize().unchecked_sub(1) })
    }

    // FIXME(const-hack) 一旦 `Ord::max` 可在 const 上下文中使用,就移除此函数
    pub(crate) const fn max(a: Self, b: Self) -> Self {
        if a.as_usize() > b.as_usize() { a } else { b }
    }
}

#[unstable(feature = "ptr_alignment_type", issue = "102070")]
impl fmt::Debug for Alignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} (1 << {:?})", self.as_nonzero(), self.log2())
    }
}

#[unstable(feature = "ptr_alignment_type", issue = "102070")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const TryFrom<NonZero<usize>> for Alignment {
    type Error = num::TryFromIntError;

    #[inline]
    fn try_from(align: NonZero<usize>) -> Result<Alignment, Self::Error> {
        align.get().try_into()
    }
}

#[unstable(feature = "ptr_alignment_type", issue = "102070")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const TryFrom<usize> for Alignment {
    type Error = num::TryFromIntError;

    #[inline]
    fn try_from(align: usize) -> Result<Alignment, Self::Error> {
        Self::new(align).ok_or(num::TryFromIntError(()))
    }
}

#[unstable(feature = "ptr_alignment_type", issue = "102070")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<Alignment> for NonZero<usize> {
    #[inline]
    fn from(align: Alignment) -> NonZero<usize> {
        align.as_nonzero()
    }
}

#[unstable(feature = "ptr_alignment_type", issue = "102070")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<Alignment> for usize {
    #[inline]
    fn from(align: Alignment) -> usize {
        align.as_usize()
    }
}

#[unstable(feature = "ptr_alignment_type", issue = "102070")]
impl cmp::Ord for Alignment {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.as_nonzero().get().cmp(&other.as_nonzero().get())
    }
}

#[unstable(feature = "ptr_alignment_type", issue = "102070")]
impl cmp::PartialOrd for Alignment {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[unstable(feature = "ptr_alignment_type", issue = "102070")]
impl hash::Hash for Alignment {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.as_nonzero().hash(state)
    }
}

/// 返回 [`Alignment::MIN`],它对任意类型都有效。
#[unstable(feature = "ptr_alignment_type", issue = "102070")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl const Default for Alignment {
    fn default() -> Alignment {
        Alignment::MIN
    }
}

#[cfg(target_pointer_width = "16")]
#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(usize)]
enum AlignmentEnum {
    _Align1Shl0 = 1 << 0,
    _Align1Shl1 = 1 << 1,
    _Align1Shl2 = 1 << 2,
    _Align1Shl3 = 1 << 3,
    _Align1Shl4 = 1 << 4,
    _Align1Shl5 = 1 << 5,
    _Align1Shl6 = 1 << 6,
    _Align1Shl7 = 1 << 7,
    _Align1Shl8 = 1 << 8,
    _Align1Shl9 = 1 << 9,
    _Align1Shl10 = 1 << 10,
    _Align1Shl11 = 1 << 11,
    _Align1Shl12 = 1 << 12,
    _Align1Shl13 = 1 << 13,
    _Align1Shl14 = 1 << 14,
    _Align1Shl15 = 1 << 15,
}

#[cfg(target_pointer_width = "32")]
#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(usize)]
enum AlignmentEnum {
    _Align1Shl0 = 1 << 0,
    _Align1Shl1 = 1 << 1,
    _Align1Shl2 = 1 << 2,
    _Align1Shl3 = 1 << 3,
    _Align1Shl4 = 1 << 4,
    _Align1Shl5 = 1 << 5,
    _Align1Shl6 = 1 << 6,
    _Align1Shl7 = 1 << 7,
    _Align1Shl8 = 1 << 8,
    _Align1Shl9 = 1 << 9,
    _Align1Shl10 = 1 << 10,
    _Align1Shl11 = 1 << 11,
    _Align1Shl12 = 1 << 12,
    _Align1Shl13 = 1 << 13,
    _Align1Shl14 = 1 << 14,
    _Align1Shl15 = 1 << 15,
    _Align1Shl16 = 1 << 16,
    _Align1Shl17 = 1 << 17,
    _Align1Shl18 = 1 << 18,
    _Align1Shl19 = 1 << 19,
    _Align1Shl20 = 1 << 20,
    _Align1Shl21 = 1 << 21,
    _Align1Shl22 = 1 << 22,
    _Align1Shl23 = 1 << 23,
    _Align1Shl24 = 1 << 24,
    _Align1Shl25 = 1 << 25,
    _Align1Shl26 = 1 << 26,
    _Align1Shl27 = 1 << 27,
    _Align1Shl28 = 1 << 28,
    _Align1Shl29 = 1 << 29,
    _Align1Shl30 = 1 << 30,
    _Align1Shl31 = 1 << 31,
}

#[cfg(target_pointer_width = "64")]
#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(usize)]
enum AlignmentEnum {
    _Align1Shl0 = 1 << 0,
    _Align1Shl1 = 1 << 1,
    _Align1Shl2 = 1 << 2,
    _Align1Shl3 = 1 << 3,
    _Align1Shl4 = 1 << 4,
    _Align1Shl5 = 1 << 5,
    _Align1Shl6 = 1 << 6,
    _Align1Shl7 = 1 << 7,
    _Align1Shl8 = 1 << 8,
    _Align1Shl9 = 1 << 9,
    _Align1Shl10 = 1 << 10,
    _Align1Shl11 = 1 << 11,
    _Align1Shl12 = 1 << 12,
    _Align1Shl13 = 1 << 13,
    _Align1Shl14 = 1 << 14,
    _Align1Shl15 = 1 << 15,
    _Align1Shl16 = 1 << 16,
    _Align1Shl17 = 1 << 17,
    _Align1Shl18 = 1 << 18,
    _Align1Shl19 = 1 << 19,
    _Align1Shl20 = 1 << 20,
    _Align1Shl21 = 1 << 21,
    _Align1Shl22 = 1 << 22,
    _Align1Shl23 = 1 << 23,
    _Align1Shl24 = 1 << 24,
    _Align1Shl25 = 1 << 25,
    _Align1Shl26 = 1 << 26,
    _Align1Shl27 = 1 << 27,
    _Align1Shl28 = 1 << 28,
    _Align1Shl29 = 1 << 29,
    _Align1Shl30 = 1 << 30,
    _Align1Shl31 = 1 << 31,
    _Align1Shl32 = 1 << 32,
    _Align1Shl33 = 1 << 33,
    _Align1Shl34 = 1 << 34,
    _Align1Shl35 = 1 << 35,
    _Align1Shl36 = 1 << 36,
    _Align1Shl37 = 1 << 37,
    _Align1Shl38 = 1 << 38,
    _Align1Shl39 = 1 << 39,
    _Align1Shl40 = 1 << 40,
    _Align1Shl41 = 1 << 41,
    _Align1Shl42 = 1 << 42,
    _Align1Shl43 = 1 << 43,
    _Align1Shl44 = 1 << 44,
    _Align1Shl45 = 1 << 45,
    _Align1Shl46 = 1 << 46,
    _Align1Shl47 = 1 << 47,
    _Align1Shl48 = 1 << 48,
    _Align1Shl49 = 1 << 49,
    _Align1Shl50 = 1 << 50,
    _Align1Shl51 = 1 << 51,
    _Align1Shl52 = 1 << 52,
    _Align1Shl53 = 1 << 53,
    _Align1Shl54 = 1 << 54,
    _Align1Shl55 = 1 << 55,
    _Align1Shl56 = 1 << 56,
    _Align1Shl57 = 1 << 57,
    _Align1Shl58 = 1 << 58,
    _Align1Shl59 = 1 << 59,
    _Align1Shl60 = 1 << 60,
    _Align1Shl61 = 1 << 61,
    _Align1Shl62 = 1 << 62,
    _Align1Shl63 = 1 << 63,
}
