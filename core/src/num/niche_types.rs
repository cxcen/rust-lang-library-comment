#![unstable(
    feature = "temporary_niche_types",
    issue = "none",
    reason = "for core, alloc, and std internals until pattern types are further along"
)]

//! 标准库内部使用的“有效范围”整数包装类型。
//!
//! 这些类型通过 `#[rustc_layout_scalar_valid_range_start]` 和
//! `#[rustc_layout_scalar_valid_range_end]` 告诉编译器：底层标量只有某个闭区间内的
//! 位模式是有效值。这个契约与 `NonZero` 的核心设计相同：如果某个位模式永远不会作为
//! 正常值出现，编译器就能把它当作 niche 存放枚举判别信息，例如让
//! `Option<NonZeroU32>` 与 `u32` 同样大小。违反这些有效范围不变量会直接破坏编译器
//! 的布局和优化假设，因此 unchecked 构造函数都是 `unsafe`。

use crate::cmp::Ordering;
use crate::fmt;
use crate::hash::{Hash, Hasher};
use crate::marker::StructuralPartialEq;

macro_rules! define_valid_range_type {
    ($(
        $(#[$m:meta])*
        $vis:vis struct $name:ident($int:ident as $uint:ident in $low:literal..=$high:literal);
    )+) => {$(
        #[derive(Clone, Copy, Eq)]
        #[repr(transparent)]
        #[rustc_layout_scalar_valid_range_start($low)]
        #[rustc_layout_scalar_valid_range_end($high)]
        $(#[$m])*
        $vis struct $name($int);

        const _: () = {
            // `valid_range` 属性按无符号数解释范围，因此这里确认无符号版本从 0 开始，
            // 并且底层有符号/无符号类型大小一致，避免属性区间和实际存储宽度脱节。
            assert!(<$uint>::MIN == 0);
            let ulow: $uint = $low;
            let uhigh: $uint = $high;
            assert!(ulow <= uhigh);

            assert!(size_of::<$int>() == size_of::<$uint>());
        };

        impl $name {
            #[inline]
            pub const fn new(val: $int) -> Option<Self> {
                if (val as $uint) >= ($low as $uint) && (val as $uint) <= ($high as $uint) {
                    // SAFETY: 上面的条件刚刚检查过 `val` 落在声明的闭区间内。
                    Some(unsafe { $name(val) })
                } else {
                    None
                }
            }

            /// 从底层整数原语构造该有效范围类型，不执行运行时检查。
            ///
            /// # 安全性(Safety）
            ///
            /// `val` 必须落在该类型声明的有效范围内。若传入范围外位模式，会立刻违反
            /// 类型有效性不变量；编译器可能已经基于这些位模式“不可能出现”的假设做布局
            /// 或分支优化，因此这种违规属于语言层面的 UB。
            #[inline]
            pub const unsafe fn new_unchecked(val: $int) -> Self {
                // SAFETY: 调用方已经承诺 `val` 位于有效范围内。
                unsafe { $name(val) }
            }

            #[inline]
            pub const fn as_inner(self) -> $int {
                // SAFETY: 该类型是透明包装，取回底层整数不会改变位模式或有效性。
                // 这里不使用 `.0`，是为了避开 MCP#807 对字段投影的限制。
                unsafe { crate::mem::transmute(self) }
            }
        }

        // 为了允许匹配常量，需要显式实现 `StructuralPartialEq`。不能依赖 derive，
        // 因为派生出的 `PartialEq` 会做字段投影，而字段投影被
        // <https://github.com/rust-lang/compiler-team/issues/807> 禁止。
        impl StructuralPartialEq for $name {}

        impl PartialEq for $name {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                self.as_inner() == other.as_inner()
            }
        }

        impl Ord for $name {
            #[inline]
            fn cmp(&self, other: &Self) -> Ordering {
                Ord::cmp(&self.as_inner(), &other.as_inner())
            }
        }

        impl PartialOrd for $name {
            #[inline]
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(Ord::cmp(self, other))
            }
        }

        impl Hash for $name {
            // 必需方法
            fn hash<H: Hasher>(&self, state: &mut H) {
                Hash::hash(&self.as_inner(), state);
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                <$int as fmt::Debug>::fmt(&self.as_inner(), f)
            }
        }
    )+};
}

define_valid_range_type! {
    pub struct Nanoseconds(u32 as u32 in 0..=999_999_999);
}

impl Nanoseconds {
    // SAFETY: 0 位于 `Nanoseconds` 声明的有效范围内。
    pub const ZERO: Self = unsafe { Nanoseconds::new_unchecked(0) };
}

#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl const Default for Nanoseconds {
    #[inline]
    fn default() -> Self {
        Self::ZERO
    }
}

define_valid_range_type! {
    pub struct NonZeroU8Inner(u8 as u8 in 1..=0xff);
    pub struct NonZeroU16Inner(u16 as u16 in 1..=0xff_ff);
    pub struct NonZeroU32Inner(u32 as u32 in 1..=0xffff_ffff);
    pub struct NonZeroU64Inner(u64 as u64 in 1..=0xffffffff_ffffffff);
    pub struct NonZeroU128Inner(u128 as u128 in 1..=0xffffffffffffffff_ffffffffffffffff);

    pub struct NonZeroI8Inner(i8 as u8 in 1..=0xff);
    pub struct NonZeroI16Inner(i16 as u16 in 1..=0xff_ff);
    pub struct NonZeroI32Inner(i32 as u32 in 1..=0xffff_ffff);
    pub struct NonZeroI64Inner(i64 as u64 in 1..=0xffffffff_ffffffff);
    pub struct NonZeroI128Inner(i128 as u128 in 1..=0xffffffffffffffff_ffffffffffffffff);

    pub struct NonZeroCharInner(char as u32 in 1..=0x10ffff);
}

#[cfg(target_pointer_width = "16")]
define_valid_range_type! {
    pub struct UsizeNoHighBit(usize as usize in 0..=0x7fff);
    pub struct NonZeroUsizeInner(usize as usize in 1..=0xffff);
    pub struct NonZeroIsizeInner(isize as usize in 1..=0xffff);
}
#[cfg(target_pointer_width = "32")]
define_valid_range_type! {
    pub struct UsizeNoHighBit(usize as usize in 0..=0x7fff_ffff);
    pub struct NonZeroUsizeInner(usize as usize in 1..=0xffff_ffff);
    pub struct NonZeroIsizeInner(isize as usize in 1..=0xffff_ffff);
}
#[cfg(target_pointer_width = "64")]
define_valid_range_type! {
    pub struct UsizeNoHighBit(usize as usize in 0..=0x7fff_ffff_ffff_ffff);
    pub struct NonZeroUsizeInner(usize as usize in 1..=0xffff_ffff_ffff_ffff);
    pub struct NonZeroIsizeInner(isize as usize in 1..=0xffff_ffff_ffff_ffff);
}

define_valid_range_type! {
    pub struct U32NotAllOnes(u32 as u32 in 0..=0xffff_fffe);
    pub struct I32NotAllOnes(i32 as u32 in 0..=0xffff_fffe);

    pub struct U64NotAllOnes(u64 as u64 in 0..=0xffff_ffff_ffff_fffe);
    pub struct I64NotAllOnes(i64 as u64 in 0..=0xffff_ffff_ffff_fffe);
}

pub trait NotAllOnesHelper {
    type Type;
}
pub type NotAllOnes<T> = <T as NotAllOnesHelper>::Type;
impl NotAllOnesHelper for u32 {
    type Type = U32NotAllOnes;
}
impl NotAllOnesHelper for i32 {
    type Type = I32NotAllOnes;
}
impl NotAllOnesHelper for u64 {
    type Type = U64NotAllOnes;
}
impl NotAllOnesHelper for i64 {
    type Type = I64NotAllOnes;
}

define_valid_range_type! {
    pub struct CodePointInner(u32 as u32 in 0..=0x10ffff);
}

impl CodePointInner {
    pub const ZERO: Self = CodePointInner::new(0).unwrap();
}

impl Default for CodePointInner {
    #[inline]
    fn default() -> Self {
        Self::ZERO
    }
}
