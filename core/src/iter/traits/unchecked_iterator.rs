use crate::iter::TrustedLen;

/// [`TrustedLen`] 不能带有方法，因此用这个 trait 为它补充内部能力。
///
/// 该 trait 目前要求 `TrustedLen`，因为除了 `TrustedLen` 之外，是否能合理地依赖
/// 其他 iterator 的 `size_hint` 仍不明确。
pub(crate) trait UncheckedIterator: TrustedLen {
    /// 从一个非空 iterator 中取得下一个元素。
    ///
    /// 因为调用前保证一定有值可返回，所以它可以直接返回 `Item` 类型，而不需要包装在
    /// `Option` 中。
    ///
    /// # 安全性(Safety）
    ///
    /// 只有在 `size_hint().0 != 0` 时才能调用。这个条件保证至少还有一个元素可用。
    ///
    /// 否则（也就是 `size_hint().1 == Some(0)` 时）调用会导致 UB。
    ///
    /// # 给实现者的说明
    ///
    /// 该方法有一个使用 [`Option::unwrap_unchecked`] 的默认实现。如果你的 `next`
    /// *总是* 返回 `Some`，例如无限 iterator，这通常已经足够。不过在更复杂的场景中，
    /// 由于 `Option` 处理，IR 中有时仍会残留 `insertvalue`/`assume`/`extractvalue`
    /// 指令；这种情况下可以考虑手动实现该方法。
    #[unstable(feature = "trusted_len_next_unchecked", issue = "37572")]
    #[inline]
    unsafe fn next_unchecked(&mut self) -> Self::Item {
        let opt = self.next();
        // SAFETY: 调用者承诺当前不为空；并且 `Self: TrustedLen`，所以可以真正信任
        // `size_hint`。
        unsafe { opt.unwrap_unchecked() }
    }
}
