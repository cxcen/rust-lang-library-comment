use crate::ascii;

impl<const N: usize> [u8; N] {
    /// 把这个字节数组转换为 ASCII 字符数组;若任意字节不是 ASCII,返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char)]
    ///
    /// const HEX_DIGITS: [std::ascii::Char; 16] =
    ///     *b"0123456789abcdef".as_ascii().unwrap();
    ///
    /// assert_eq!(HEX_DIGITS[1].as_str(), "1");
    /// assert_eq!(HEX_DIGITS[10].as_str(), "a");
    /// ```
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[must_use]
    #[inline]
    pub const fn as_ascii(&self) -> Option<&[ascii::Char; N]> {
        if self.is_ascii() {
            // SAFETY: 上面刚检查过整个数组都是 ASCII。
            Some(unsafe { self.as_ascii_unchecked() })
        } else {
            None
        }
    }

    /// 把这个字节数组转换为 ASCII 字符数组,但不检查这些字节是否有效。
    ///
    /// # 安全性(Safety）
    ///
    /// 数组中的每个字节都必须位于 `0..=127`;否则会构造出无效的 `ascii::Char`,导致 UB。
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[must_use]
    #[inline]
    pub const unsafe fn as_ascii_unchecked(&self) -> &[ascii::Char; N] {
        let byte_ptr: *const [u8; N] = self;
        let ascii_ptr = byte_ptr as *const [ascii::Char; N];
        // SAFETY: 调用方承诺所有字节都是 ASCII。
        unsafe { &*ascii_ptr }
    }
}
