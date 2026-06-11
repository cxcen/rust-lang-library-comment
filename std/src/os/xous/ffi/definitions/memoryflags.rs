/// 要传递给 MapMemory struct 的标志（flags）。
/// 注意：让内存可写（writable）却不可读（readable）是一种错误。
#[derive(Copy, PartialEq, Eq, Clone, PartialOrd, Ord, Hash, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct MemoryFlags {
    bits: usize,
}

impl MemoryFlags {
    /// 释放（free）这块内存
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const FREE: Self = Self { bits: 0b0000_0000 };

    /// 立即分配这块内存。否则它将按需分页（demand-paged）。当 `phys` 不为 0 时，
    /// 该标志会被隐式设置。
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const RESERVE: Self = Self { bits: 0b0000_0001 };

    /// 允许 CPU 从此页（page）读取。
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const R: Self = Self { bits: 0b0000_0010 };

    /// 允许 CPU 向此页（page）写入。
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const W: Self = Self { bits: 0b0000_0100 };

    /// 允许 CPU 从此页（page）执行。
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const X: Self = Self { bits: 0b0000_1000 };

    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn bits(&self) -> usize {
        self.bits
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn from_bits(raw: usize) -> Option<MemoryFlags> {
        if raw > 16 { None } else { Some(MemoryFlags { bits: raw }) }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn empty() -> MemoryFlags {
        MemoryFlags { bits: 0 }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn all() -> MemoryFlags {
        MemoryFlags { bits: 15 }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl core::fmt::Binary for MemoryFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Binary::fmt(&self.bits, f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl core::fmt::Octal for MemoryFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Octal::fmt(&self.bits, f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl core::fmt::LowerHex for MemoryFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::LowerHex::fmt(&self.bits, f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl core::fmt::UpperHex for MemoryFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::UpperHex::fmt(&self.bits, f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl core::ops::BitOr for MemoryFlags {
    type Output = Self;

    /// 返回这两组标志的并集（union）。
    #[inline]
    fn bitor(self, other: MemoryFlags) -> Self {
        Self { bits: self.bits | other.bits }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl core::ops::BitOrAssign for MemoryFlags {
    /// 加入这组标志。
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl core::ops::BitXor for MemoryFlags {
    type Output = Self;

    /// 返回左侧标志，但其中所有右侧标志位都被翻转（toggle）。
    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self { bits: self.bits ^ other.bits }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl core::ops::BitXorAssign for MemoryFlags {
    /// 翻转（toggle）这组标志。
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl core::ops::BitAnd for MemoryFlags {
    type Output = Self;

    /// 返回这两组标志之间的交集（intersection）。
    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self { bits: self.bits & other.bits }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl core::ops::BitAndAssign for MemoryFlags {
    /// 禁用集合中所有被禁用的标志。
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl core::ops::Sub for MemoryFlags {
    type Output = Self;

    /// 返回这两组标志的集合差（set difference）。
    #[inline]
    fn sub(self, other: Self) -> Self {
        Self { bits: self.bits & !other.bits }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl core::ops::SubAssign for MemoryFlags {
    /// 禁用集合中所有已启用的标志。
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl core::ops::Not for MemoryFlags {
    type Output = Self;

    /// 返回这组标志的补集（complement）。
    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits } & MemoryFlags { bits: 15 }
    }
}
