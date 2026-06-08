// 包含打印有用 `assert!` 消息所需的内部机制。它不面向公开使用，
// 甚至也不面向 nightly 用户直接使用。
//
// 基于 https://github.com/dtolnay/case-studies/tree/master/autoref-specialization。
// 当 'specialization' 足够健壮时（5 年？10 年？也许永远不会？），
// `Capture` 就可以特化到 [Printable]。

#![allow(missing_debug_implementations)]
#![doc(hidden)]
#![unstable(feature = "generic_assert_internals", issue = "44838")]

use crate::fmt::{Debug, Formatter};
use crate::marker::PhantomData;

// ***** TryCapture - 泛型 *****

/// [Capture] 使用的标记类型。
#[unstable(feature = "generic_assert_internals", issue = "44838")]
pub struct TryCaptureWithoutDebug;

/// 捕获任意 `E`，并据此修改 `to`。
#[unstable(feature = "generic_assert_internals", issue = "44838")]
pub trait TryCaptureGeneric<E, M> {
    /// 类似 [TryCapturePrintable]，但泛化到任意 `E`。
    fn try_capture(&self, to: &mut Capture<E, M>);
}

impl<E> TryCaptureGeneric<E, TryCaptureWithoutDebug> for &Wrapper<&E> {
    #[inline]
    fn try_capture(&self, _: &mut Capture<E, TryCaptureWithoutDebug>) {}
}

impl<E> Debug for Capture<E, TryCaptureWithoutDebug> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        f.write_str("N/A")
    }
}

// ***** TryCapture - 可打印 *****

/// [Capture] 使用的标记类型。
#[unstable(feature = "generic_assert_internals", issue = "44838")]
pub struct TryCaptureWithDebug;

/// 捕获任意 `E: Printable`，并据此修改 `to`。
#[unstable(feature = "generic_assert_internals", issue = "44838")]
pub trait TryCapturePrintable<E, M> {
    /// 类似 [TryCaptureGeneric]，但特化到任意 `E: Printable`。
    fn try_capture(&self, to: &mut Capture<E, M>);
}

impl<E> TryCapturePrintable<E, TryCaptureWithDebug> for Wrapper<&E>
where
    E: Printable,
{
    #[inline]
    fn try_capture(&self, to: &mut Capture<E, TryCaptureWithDebug>) {
        to.elem = Some(*self.0);
    }
}

impl<E> Debug for Capture<E, TryCaptureWithDebug>
where
    E: Printable,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self.elem {
            None => f.write_str("N/A"),
            Some(ref value) => Debug::fmt(value, f),
        }
    }
}

// ***** 其他 *****

//spellchecker:off
/// 所有可能捕获的 `assert!` 元素。
///
/// # 类型
///
/// * `E`: 将要显示的元素。
/// * `M`: 用于按 [Debug] 区分 [Capture] 的标记。
//spellchecker:on
#[unstable(feature = "generic_assert_internals", issue = "44838")]
pub struct Capture<E, M> {
    // 如果为 None，则 `E` 没有实现 [Printable]，或者 `E` 未被求值
    // （`assert!( ... )` 短路）。
    //
    // 如果为 Some，则 `E` 实现了 [Printable] 且已被求值。
    pub elem: Option<E>,
    phantom: PhantomData<M>,
}

impl<M, T> Capture<M, T> {
    #[inline]
    pub const fn new() -> Self {
        Self { elem: None, phantom: PhantomData }
    }
}

/// 实现 `TryCapture*` 时需要。
#[unstable(feature = "generic_assert_internals", issue = "44838")]
pub struct Wrapper<T>(pub T);

/// 表示哪些元素可以被复制和显示。
#[unstable(feature = "generic_assert_internals", issue = "44838")]
pub trait Printable: Copy + Debug {}

impl<T> Printable for T where T: Copy + Debug {}
