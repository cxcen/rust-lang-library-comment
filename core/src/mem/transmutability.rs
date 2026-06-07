use crate::marker::ConstParamTy_;

/// 标记 `Src` 可以被 transmute（重解释）为 `Self`。
///
/// # 实现（Implementation）
///
/// 此 trait 无法被显式实现。它由编译器即时（on-the-fly）地为所有满足以下条件的类型 `Src`
/// 与 `Self` 实现：在给定一组施加于程序员身上的安全义务（参见 [`Assume`]）的前提下，
/// 编译器已经证明类型 `Src` 的值的各个位（bits）可以被健全地（soundly）重解释为一个 `Self`。
///
/// # 安全性（Safety）
///
/// 如果 `Dst: TransmuteFrom<Src, ASSUMPTIONS>`，那么只要程序员保证给定的
/// [`ASSUMPTIONS`](Assume) 得到满足，编译器就保证 `Src` 可以被健全地
/// *经由 union 进行 transmute（union-transmutable）*为一个类型 `Dst` 的值。
///
/// 一个 union-transmute 是任何形如下面这种形式的位重解释（bit-reinterpretation）转换：
///
/// ```rust
/// pub unsafe fn transmute_via_union<Src, Dst>(src: Src) -> Dst {
///     use core::mem::ManuallyDrop;
///
///     #[repr(C)]
///     union Transmute<Src, Dst> {
///         src: ManuallyDrop<Src>,
///         dst: ManuallyDrop<Dst>,
///     }
///
///     let transmute = Transmute {
///         src: ManuallyDrop::new(src),
///     };
///
///     let dst = unsafe { transmute.dst };
///
///     ManuallyDrop::into_inner(dst)
/// }
/// ```
///
/// 注意，这种构造比 [`mem::transmute_copy`](super::transmute_copy) 更宽松；union-transmute
/// 允许这样的转换：用尾部填充（trailing padding）扩展 `Src` 的位，以填满 `Self` 尾部那些
/// 未初始化的字节；例如：
///
/// ```rust
/// #![feature(transmutability)]
///
/// use core::mem::{Assume, TransmuteFrom};
///
/// let src = 42u8; // size = 1
///
/// #[repr(C, align(2))]
/// struct Dst(u8); // size = 2
//
/// let _ = unsafe {
///     <Dst as TransmuteFrom<u8, { Assume::SAFETY }>>::transmute(src)
/// };
/// ```
///
/// # 注意事项（Caveats）
///
/// ## 可移植性（Portability）
///
/// 此 trait 的实现不提供任何跨工具链、跨目标平台或跨编译的可移植性保证。此 trait 可能在某些
/// 工具链、目标平台或编译下为 `Src`、`Self` 与 `ASSUME` 的某些组合实现，而在另一些下则不实现。
/// 例如，如果 `Src` 或 `Self` 的布局是非确定性的，那么此 trait 实现的存在与否也可能是非确定性的。
/// 即便 `Src` 与 `Self` 拥有确定性的布局（例如它们是 `repr(C)` 结构体），Rust 也并未规定其原生
/// 整数类型的对齐方式，而涉及这些类型的布局可能在不同工具链、目标平台或编译之间有所不同。
///
/// ## 稳定性（Stability）
///
/// 此 trait 的实现不提供任何跨“定义 `Src` 与 `Self` 类型的 crate 版本”的 SemVer 稳定性保证。
/// 如果 SemVer 稳定性对你的应用至关重要，你必须查阅 `Src` 与 `Self` 所属定义 crate 的文档。
/// 注意，仅有 `repr(C)` 本身并不携带 SemVer 稳定性这一安全不变量。此外，稳定性并不蕴含可移植性。
/// 例如，`usize` 的大小是稳定的，但并不可移植。
#[unstable(feature = "transmutability", issue = "99571")]
#[unstable_feature_bound(transmutability)]
#[lang = "transmute_trait"]
#[rustc_deny_explicit_impl]
#[rustc_do_not_implement_via_object]
#[rustc_coinductive]
pub unsafe trait TransmuteFrom<Src, const ASSUME: Assume = { Assume::NOTHING }>
where
    Src: ?Sized,
{
    /// 把一个 `Src` 值 transmute 为一个 `Self`。
    ///
    /// # 安全性（Safety）
    ///
    /// 调用方的安全义务取决于 `ASSUME` 的取值：
    /// - 如果 [`ASSUME.alignment`](Assume::alignment)，调用方必须保证返回的 `Self` 中各个引用的
    ///   地址满足其被引用类型（referent type）的对齐要求。
    /// - 如果 [`ASSUME.lifetimes`](Assume::lifetimes)，调用方必须保证返回的 `Self` 中的引用
    ///   不会活得比它们的被引用者（referent）更久。
    /// - 如果 [`ASSUME.safety`](Assume::safety)，返回的值可能不满足 `Self` 的库级安全不变量
    ///   （library safety invariants），调用方必须保证使用返回值不会引发未定义行为。
    /// - 如果 [`ASSUME.validity`](Assume::validity)，调用方必须保证 `src` 是 `Self` 的一个
    ///   位有效（bit-valid）的实例。
    ///
    /// 在满足上述义务（如果有的话）时，调用方*绝不*能假定此 trait 提供任何固有的布局
    /// [可移植性](#portability)或[稳定性](#stability)保证。
    unsafe fn transmute(src: Src) -> Self
    where
        Src: Sized,
        Self: Sized,
    {
        use super::ManuallyDrop;

        #[repr(C)]
        union Transmute<Src, Dst> {
            src: ManuallyDrop<Src>,
            dst: ManuallyDrop<Dst>,
        }

        let transmute = Transmute { src: ManuallyDrop::new(src) };

        // SAFETY: 把 `src` 的各个位重解释为一个类型 `Self` 的值是安全的，因为：结合此 trait 上的
        // 不变量与施加于调用方身上的约定（contract），`src` 已被证明同时满足 `Self` 的语言级
        // 不变量与库级不变量。对于所有未被调用方 `ASSUME` 的不变量，其安全义务由编译器提供；
        // 反之，对于所有被调用方 `ASSUME` 的不变量，其安全义务则由施加于调用方身上的约定提供。
        let dst = unsafe { transmute.dst };

        ManuallyDrop::into_inner(dst)
    }
}

/// [`TransmuteFrom`] 的可配置证明假设（proof assumptions）。
///
/// 当为 `false` 时，相应的证明义务归属于编译器。当为 `true` 时，
/// 安全证明的责任归属于程序员。
#[unstable(feature = "transmutability", issue = "99571")]
#[lang = "transmute_opts"]
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct Assume {
    /// 当为 `false` 时，对于那些可能违反引用对齐要求的 transmute，[`TransmuteFrom`] 不会被实现；
    /// 例如：
    ///
    /// ```compile_fail,E0277
    /// #![feature(transmutability)]
    /// use core::mem::TransmuteFrom;
    ///
    /// assert_eq!(align_of::<[u8; 2]>(), 1);
    /// assert_eq!(align_of::<u16>(), 2);
    ///
    /// let src: &[u8; 2] = &[0xFF, 0xFF];
    ///
    /// // SAFETY: 无安全义务。
    /// let dst: &u16 = unsafe {
    ///     <_ as TransmuteFrom<_>>::transmute(src)
    /// };
    /// ```
    ///
    /// 当为 `true` 时，[`TransmuteFrom`] 会假定*你*已确保 transmute 后的值中的引用满足其被引用
    /// 类型的对齐要求；例如：
    ///
    /// ```rust
    /// #![feature(pointer_is_aligned_to, transmutability)]
    /// use core::mem::{Assume, TransmuteFrom};
    ///
    /// let src: &[u8; 2] = &[0xFF, 0xFF];
    ///
    /// let maybe_dst: Option<&u16> = if <*const _>::is_aligned_to(src, align_of::<u16>()) {
    ///     // SAFETY: 我们已在上面检查过 `src` 的地址满足 `u16` 的对齐要求。
    ///     Some(unsafe {
    ///         <_ as TransmuteFrom<_, { Assume::ALIGNMENT }>>::transmute(src)
    ///     })
    /// } else {
    ///     None
    /// };
    ///
    /// assert!(matches!(maybe_dst, Some(&u16::MAX) | None));
    /// ```
    pub alignment: bool,

    /// 当为 `false` 时，对于那些会延长引用生命周期的 transmute，[`TransmuteFrom`] 不会被实现。
    ///
    /// 当为 `true` 时，[`TransmuteFrom`] 会假定*你*已确保 transmute 后的值中的引用
    /// 不会活得比它们的被引用者更久。
    pub lifetimes: bool,

    /// 当为 `false` 时，对于那些可能违反目标类型库级安全不变量的 transmute，[`TransmuteFrom`]
    /// 不会被实现；例如：
    ///
    /// ```compile_fail,E0277
    /// #![feature(transmutability)]
    /// use core::mem::TransmuteFrom;
    ///
    /// let src: u8 = 3;
    ///
    /// struct EvenU8 {
    ///     // SAFETY: `val` 必须是一个偶数。
    ///     val: u8,
    /// }
    ///
    /// // SAFETY: 无安全义务。
    /// let dst: EvenU8 = unsafe {
    ///     <_ as TransmuteFrom<_>>::transmute(src)
    /// };
    /// ```
    ///
    /// 当为 `true` 时，[`TransmuteFrom`] 会假定*你*已确保使用 transmute 后的值不会引发未定义行为；
    /// 例如：
    ///
    /// ```rust
    /// #![feature(transmutability)]
    /// use core::mem::{Assume, TransmuteFrom};
    ///
    /// let src: u8 = 42;
    ///
    /// struct EvenU8 {
    ///     // SAFETY: `val` 必须是一个偶数。
    ///     val: u8,
    /// }
    ///
    /// let maybe_dst: Option<EvenU8> = if src % 2 == 0 {
    ///     // SAFETY: 我们已在上面检查过 `src` 的值是偶数。
    ///     Some(unsafe {
    ///         <_ as TransmuteFrom<_, { Assume::SAFETY }>>::transmute(src)
    ///     })
    /// } else {
    ///     None
    /// };
    ///
    /// assert!(matches!(maybe_dst, Some(EvenU8 { val: 42 })));
    /// ```
    pub safety: bool,

    /// 当为 `false` 时，对于那些可能违反目标类型语言级位有效性不变量（bit-validity invariant）的
    /// transmute，[`TransmuteFrom`] 不会被实现；例如：
    ///
    /// ```compile_fail,E0277
    /// #![feature(transmutability)]
    /// use core::mem::TransmuteFrom;
    ///
    /// let src: u8 = 3;
    ///
    /// // SAFETY: 无安全义务。
    /// let dst: bool = unsafe {
    ///     <_ as TransmuteFrom<_>>::transmute(src)
    /// };
    /// ```
    ///
    /// 当为 `true` 时，[`TransmuteFrom`] 会假定*你*已确保被 transmute 的值是 transmute 后类型的
    /// 一个位有效（bit-valid）实例；例如：
    ///
    /// ```rust
    /// #![feature(transmutability)]
    /// use core::mem::{Assume, TransmuteFrom};
    ///
    /// let src: u8 = 1;
    ///
    /// let maybe_dst: Option<bool> = if src == 0 || src == 1 {
    ///     // SAFETY: 我们已在上面检查过 `src` 的值是 `bool` 的一个位有效实例。
    ///     Some(unsafe {
    ///         <_ as TransmuteFrom<_, { Assume::VALIDITY }>>::transmute(src)
    ///     })
    /// } else {
    ///     None
    /// };
    ///
    /// assert_eq!(maybe_dst, Some(true));
    /// ```
    pub validity: bool,
}

#[unstable(feature = "transmutability", issue = "99571")]
#[unstable_feature_bound(transmutability)]
impl ConstParamTy_ for Assume {}

impl Assume {
    /// 使用它时，[`TransmuteFrom`] 不会假定你已确保满足任何安全义务，
    /// 而是仅依靠它自身的分析来（证明或反证）transmute 的可行性。
    #[unstable(feature = "transmutability", issue = "99571")]
    pub const NOTHING: Self =
        Self { alignment: false, lifetimes: false, safety: false, validity: false };

    /// 使用它时，[`TransmuteFrom`] 仅假定你已确保 transmute 后的值中的引用满足其被引用类型的
    /// 对齐要求。示例参见 [`Assume::alignment`]。
    #[unstable(feature = "transmutability", issue = "99571")]
    pub const ALIGNMENT: Self = Self { alignment: true, ..Self::NOTHING };

    /// 使用它时，[`TransmuteFrom`] 仅假定你已确保 transmute 后的值中的引用不会活得比它们的
    /// 被引用者更久。示例参见 [`Assume::lifetimes`]。
    #[unstable(feature = "transmutability", issue = "99571")]
    pub const LIFETIMES: Self = Self { lifetimes: true, ..Self::NOTHING };

    /// 使用它时，[`TransmuteFrom`] 仅假定你已确保使用 transmute 后的值不会引发未定义行为。
    /// 示例参见 [`Assume::safety`]。
    #[unstable(feature = "transmutability", issue = "99571")]
    pub const SAFETY: Self = Self { safety: true, ..Self::NOTHING };

    /// 使用它时，[`TransmuteFrom`] 仅假定你已确保被 transmute 的值是 transmute 后类型的一个
    /// 位有效实例。示例参见 [`Assume::validity`]。
    #[unstable(feature = "transmutability", issue = "99571")]
    pub const VALIDITY: Self = Self { validity: true, ..Self::NOTHING };

    /// 合并 `self` 与 `other_assumptions` 的假设。
    ///
    /// 这在泛型上下文中扩展 [`Assume`] 时尤为有用；例如：
    ///
    /// ```rust
    /// #![feature(
    ///     adt_const_params,
    ///     generic_const_exprs,
    ///     pointer_is_aligned_to,
    ///     transmutability,
    /// )]
    /// #![allow(incomplete_features)]
    /// use core::mem::{Assume, TransmuteFrom};
    ///
    /// /// 尝试把 `src` transmute 为 `&Dst`。
    /// ///
    /// /// 如果 `src` 违反了 `&Dst` 的对齐要求，则返回 `None`。
    /// ///
    /// /// # Safety
    /// ///
    /// /// 调用方保证 `ASSUME` 所要求的义务（对齐除外）均已满足。
    /// unsafe fn try_transmute_ref<'a, Src, Dst, const ASSUME: Assume>(src: &'a Src) -> Option<&'a Dst>
    /// where
    ///     &'a Dst: TransmuteFrom<&'a Src, { ASSUME.and(Assume::ALIGNMENT) }>,
    /// {
    ///     if <*const _>::is_aligned_to(src, align_of::<Dst>()) {
    ///         // SAFETY: 通过上面的动态检查，我们已确保 `src` 的地址满足 `&Dst` 的对齐要求。
    ///         // 而根据施加于调用方身上的约定，`ASSUME` 所要求的安全义务也已得到满足。
    ///         Some(unsafe {
    ///             <_ as TransmuteFrom<_, { ASSUME.and(Assume::ALIGNMENT) }>>::transmute(src)
    ///         })
    ///     } else {
    ///         None
    ///     }
    /// }
    ///
    /// let src: &[u8; 2] = &[0xFF, 0xFF];
    ///
    /// // SAFETY: 无安全义务。
    /// let maybe_dst: Option<&u16> = unsafe {
    ///     try_transmute_ref::<_, _, { Assume::NOTHING }>(src)
    /// };
    ///```
    #[unstable(feature = "transmutability", issue = "99571")]
    pub const fn and(self, other_assumptions: Self) -> Self {
        Self {
            alignment: self.alignment || other_assumptions.alignment,
            lifetimes: self.lifetimes || other_assumptions.lifetimes,
            safety: self.safety || other_assumptions.safety,
            validity: self.validity || other_assumptions.validity,
        }
    }

    /// 从 `self` 的义务中移除 `other_assumptions`；例如：
    ///
    /// ```rust
    /// #![feature(transmutability)]
    /// use core::mem::Assume;
    ///
    /// let assumptions = Assume::ALIGNMENT.and(Assume::SAFETY);
    /// let to_be_removed = Assume::SAFETY.and(Assume::VALIDITY);
    ///
    /// assert_eq!(
    ///     assumptions.but_not(to_be_removed),
    ///     Assume::ALIGNMENT,
    /// );
    /// ```
    #[unstable(feature = "transmutability", issue = "99571")]
    pub const fn but_not(self, other_assumptions: Self) -> Self {
        Self {
            alignment: self.alignment && !other_assumptions.alignment,
            lifetimes: self.lifetimes && !other_assumptions.lifetimes,
            safety: self.safety && !other_assumptions.safety,
            validity: self.validity && !other_assumptions.validity,
        }
    }
}

// FIXME(jswrenn): 这个 const 运算其实无法使用。为什么？
// https://github.com/rust-lang/rust/pull/100726#issuecomment-1219928926
#[unstable(feature = "transmutability", issue = "99571")]
impl core::ops::Add for Assume {
    type Output = Assume;

    fn add(self, other_assumptions: Assume) -> Assume {
        self.and(other_assumptions)
    }
}

// FIXME(jswrenn): 这个 const 运算其实无法使用。为什么？
// https://github.com/rust-lang/rust/pull/100726#issuecomment-1219928926
#[unstable(feature = "transmutability", issue = "99571")]
impl core::ops::Sub for Assume {
    type Output = Assume;

    fn sub(self, other_assumptions: Assume) -> Assume {
        self.but_not(other_assumptions)
    }
}
