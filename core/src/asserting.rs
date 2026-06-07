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

// ***** TryCapture - Generic *****

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

// ***** TryCapture - Printable *****

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

// ***** Others *****

//spellchecker:off
/// All possible captured `assert!` elements
///
/// # Types
///
/// * `E`: **E**lement that is going to be displayed.
/// * `M`: **M**arker used to differentiate [Capture]s in regards to [Debug].
//spellchecker:on
#[unstable(feature = "generic_assert_internals", issue = "44838")]
pub struct Capture<E, M> {
    // If None, then `E` does not implements [Printable] or `E` wasn't evaluated (`assert!( ... )`
    // short-circuited).
    //
    // If Some, then `E` implements [Printable] and was evaluated.
    pub elem: Option<E>,
    phantom: PhantomData<M>,
}

impl<M, T> Capture<M, T> {
    #[inline]
    pub const fn new() -> Self {
        Self { elem: None, phantom: PhantomData }
    }
}

/// Necessary for the implementations of `TryCapture*`
#[unstable(feature = "generic_assert_internals", issue = "44838")]
pub struct Wrapper<T>(pub T);

/// Tells which elements can be copied and displayed
#[unstable(feature = "generic_assert_internals", issue = "44838")]
pub trait Printable: Copy + Debug {}

impl<T> Printable for T where T: Copy + Debug {}
