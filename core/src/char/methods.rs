//! `char` 的固有方法实现。

use super::*;
use crate::panic::const_panic;
use crate::slice;
use crate::str::from_utf8_unchecked_mut;
use crate::ub_checks::assert_unsafe_precondition;
use crate::unicode::printable::is_printable;
use crate::unicode::{self, conversions};

impl char {
    /// `char` 可具有的最低有效 code point，`'\0'`。
    ///
    /// 与整数类型不同，`char` 的取值空间中间存在一个空洞：UTF-16 代理项范围
    /// U+D800..=U+DFFF 不是 Unicode scalar value，因此不能由 `char` 表示。
    /// 这意味着可能的 `char` 数量少于从 `MIN` 到 [`MAX`] 的整数距离。
    /// `char` 范围迭代会自动跳过这个空洞：
    ///
    /// ```
    /// let dist = u32::from(char::MAX) - u32::from(char::MIN);
    /// let size = (char::MIN..=char::MAX).count() as u32;
    /// assert!(size < dist);
    /// ```
    ///
    /// 尽管中间存在该空洞，`MIN` 和 [`MAX`] 仍可作为所有 `char` 值的上下界。
    ///
    /// [`MAX`]: char::MAX
    ///
    /// # 示例
    ///
    /// ```
    /// # fn something_which_returns_char() -> char { 'a' }
    /// let c: char = something_which_returns_char();
    /// assert!(char::MIN <= c);
    ///
    /// let value_at_min = u32::from(char::MIN);
    /// assert_eq!(char::from_u32(value_at_min), Some('\0'));
    /// ```
    #[stable(feature = "char_min", since = "1.83.0")]
    pub const MIN: char = '\0';

    /// `char` 可具有的最高有效 code point，`'\u{10FFFF}'`。
    ///
    /// 与整数类型不同，`char` 的取值空间中间存在一个空洞：UTF-16 代理项范围
    /// U+D800..=U+DFFF 不是 Unicode scalar value，因此不能由 `char` 表示。
    /// 这意味着可能的 `char` 数量少于从 [`MIN`] 到 `MAX` 的整数距离。
    /// `char` 范围迭代会自动跳过这个空洞：
    ///
    /// ```
    /// let dist = u32::from(char::MAX) - u32::from(char::MIN);
    /// let size = (char::MIN..=char::MAX).count() as u32;
    /// assert!(size < dist);
    /// ```
    ///
    /// 尽管中间存在该空洞，[`MIN`] 和 `MAX` 仍可作为所有 `char` 值的上下界。
    ///
    /// [`MIN`]: char::MIN
    ///
    /// # 示例
    ///
    /// ```
    /// # fn something_which_returns_char() -> char { 'a' }
    /// let c: char = something_which_returns_char();
    /// assert!(c <= char::MAX);
    ///
    /// let value_at_max = u32::from(char::MAX);
    /// assert_eq!(char::from_u32(value_at_max), Some('\u{10FFFF}'));
    /// assert_eq!(char::from_u32(value_at_max + 1), None);
    /// ```
    #[stable(feature = "assoc_char_consts", since = "1.52.0")]
    pub const MAX: char = '\u{10FFFF}';

    /// 将 `char` [编码](char::encode_utf8)为 UTF-8 时最多需要的字节数。
    ///
    /// 由于 `char` 是 Unicode scalar value，UTF-8 编码长度最多为 4 字节。
    #[stable(feature = "char_max_len_assoc", since = "1.93.0")]
    pub const MAX_LEN_UTF8: usize = 4;

    /// 将 `char` [编码](char::encode_utf16)为 UTF-16 时最多需要的 2 字节 code unit 数。
    ///
    /// BMP 内的 scalar value 需要 1 个 code unit，补充平面中的 scalar value 需要一个代理对，
    /// 即 2 个 code unit。
    #[stable(feature = "char_max_len_assoc", since = "1.93.0")]
    pub const MAX_LEN_UTF16: usize = 2;

    /// `U+FFFD REPLACEMENT CHARACTER`（�）在 Unicode 中用于表示解码错误。
    ///
    /// 例如，把非良构 UTF-8 字节交给
    /// [`String::from_utf8_lossy`](../std/string/struct.String.html#method.from_utf8_lossy)
    /// 时，非法片段会被替换为该字符。
    #[stable(feature = "assoc_char_consts", since = "1.52.0")]
    pub const REPLACEMENT_CHARACTER: char = '\u{FFFD}';

    /// `char` 和 `str` 中 Unicode 相关方法所依据的
    /// [Unicode](https://www.unicode.org/) 版本。
    ///
    /// Unicode 会定期发布新版本，标准库中依赖 Unicode 数据的方法也会随之更新。
    /// 因此，部分 `char` 和 `str` 方法的行为以及该常量的值会随时间变化；
    /// 这种随 Unicode 标准演进而发生的变化不视为破坏性变更。
    ///
    /// 版本号规则见
    /// [Unicode 11.0 或更高版本，3.1 节 Versions of the Unicode Standard](https://www.unicode.org/versions/Unicode11.0.0/ch03.pdf#page=4)。
    #[stable(feature = "assoc_char_consts", since = "1.52.0")]
    pub const UNICODE_VERSION: (u8, u8, u8) = crate::unicode::UNICODE_VERSION;

    /// 为 `iter` 中按本机端序表示的 UTF-16 code unit 创建解码迭代器，
    /// 遇到未配对代理项时返回 `Err`。
    ///
    /// 合法代理对会合成为一个 `char`。单独的前导代理项或尾随代理项都不是
    /// Unicode scalar value，因此不能作为 `char` 产生，只能通过错误暴露给调用方。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// // 𝄞mus<invalid>ic<invalid>
    /// let v = [
    ///     0xD834, 0xDD1E, 0x006d, 0x0075, 0x0073, 0xDD1E, 0x0069, 0x0063, 0xD834,
    /// ];
    ///
    /// assert_eq!(
    ///     char::decode_utf16(v)
    ///         .map(|r| r.map_err(|e| e.unpaired_surrogate()))
    ///         .collect::<Vec<_>>(),
    ///     vec![
    ///         Ok('𝄞'),
    ///         Ok('m'), Ok('u'), Ok('s'),
    ///         Err(0xDD1E),
    ///         Ok('i'), Ok('c'),
    ///         Err(0xD834)
    ///     ]
    /// );
    /// ```
    ///
    /// 把 `Err` 结果替换为替换字符，就可以得到有损解码器：
    ///
    /// ```
    /// // 𝄞mus<invalid>ic<invalid>
    /// let v = [
    ///     0xD834, 0xDD1E, 0x006d, 0x0075, 0x0073, 0xDD1E, 0x0069, 0x0063, 0xD834,
    /// ];
    ///
    /// assert_eq!(
    ///     char::decode_utf16(v)
    ///        .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
    ///        .collect::<String>(),
    ///     "𝄞mus�ic�"
    /// );
    /// ```
    #[stable(feature = "assoc_char_funcs", since = "1.52.0")]
    #[inline]
    pub fn decode_utf16<I: IntoIterator<Item = u16>>(iter: I) -> DecodeUtf16<I::IntoIter> {
        super::decode::decode_utf16(iter)
    }

    /// 将 `u32` 转换为 `char`。
    ///
    /// 所有 `char` 都能作为 [`u32`] 表示，并可用
    /// [`as`](../std/keyword.as.html) 转换为数值 code point：
    ///
    /// ```
    /// let c = '💯';
    /// let i = c as u32;
    ///
    /// assert_eq!(128175, i);
    /// ```
    ///
    /// 反过来并不成立：不是所有 [`u32`] 都是合法 `char`。如果输入超过 U+10FFFF，
    /// 或位于 UTF-16 代理项范围 U+D800..=U+DFFF，`from_u32()` 会返回 `None`。
    ///
    /// 如需忽略这些检查的 unsafe 版本，见 [`from_u32_unchecked`]。
    ///
    /// [`from_u32_unchecked`]: #method.from_u32_unchecked
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let c = char::from_u32(0x2764);
    ///
    /// assert_eq!(Some('❤'), c);
    /// ```
    ///
    /// 当输入不是合法 `char` 时返回 `None`：
    ///
    /// ```
    /// let c = char::from_u32(0x110000);
    ///
    /// assert_eq!(None, c);
    /// ```
    #[stable(feature = "assoc_char_funcs", since = "1.52.0")]
    #[rustc_const_stable(feature = "const_char_convert", since = "1.67.0")]
    #[must_use]
    #[inline]
    pub const fn from_u32(i: u32) -> Option<char> {
        super::convert::from_u32(i)
    }

    /// 忽略有效性检查，将 `u32` 转换为 `char`。
    ///
    /// 所有 `char` 都能作为 [`u32`] 表示，并可用 `as` 转换为数值 code point：
    ///
    /// ```
    /// let c = '💯';
    /// let i = c as u32;
    ///
    /// assert_eq!(128175, i);
    /// ```
    ///
    /// 反过来并不成立：不是所有 [`u32`] 都是合法 `char`。`from_u32_unchecked()`
    /// 会跳过检查并直接构造 `char`，因此可能制造无效值。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须保证 `i` 是 Unicode scalar value：`i <= 0x10FFFF`，
    /// 且 `i` 不在代理项范围 `0xD800..=0xDFFF` 内。
    ///
    /// 违反该前置条件会构造无效 `char`。`char` 的有效性是编译器和标准库依赖的类型不变量；
    /// 一旦被破坏，后续模式匹配、编码或优化都可能在错误假设下运行并导致 UB。
    ///
    /// 安全版本见 [`from_u32`]。
    ///
    /// [`from_u32`]: #method.from_u32
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let c = unsafe { char::from_u32_unchecked(0x2764) };
    ///
    /// assert_eq!('❤', c);
    /// ```
    #[stable(feature = "assoc_char_funcs", since = "1.52.0")]
    #[rustc_const_stable(feature = "const_char_from_u32_unchecked", since = "1.81.0")]
    #[must_use]
    #[inline]
    pub const unsafe fn from_u32_unchecked(i: u32) -> char {
        // SAFETY: 调用方必须维护 `from_u32_unchecked` 的契约：
        // `i` 是合法 Unicode scalar value，不越界且不是代理项。
        unsafe { super::convert::from_u32_unchecked(i) }
    }

    /// 将给定基数中的数字转换为 `char`。
    ///
    /// 这里的 radix 也常称为 base：2 表示二进制，10 表示十进制，16 表示十六进制。
    /// 支持任意 2 到 36 之间的基数。
    ///
    /// 如果 `num` 不是给定基数中的一位数字，`from_digit()` 返回 `None`。
    ///
    /// # Panics
    ///
    /// 当给定基数大于 36 时 panic。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let c = char::from_digit(4, 10);
    ///
    /// assert_eq!(Some('4'), c);
    ///
    /// // 十进制 11 是 16 进制中的一位数字。
    /// let c = char::from_digit(11, 16);
    ///
    /// assert_eq!(Some('b'), c);
    /// ```
    ///
    /// 当输入不是该基数中的数字时返回 `None`：
    ///
    /// ```
    /// let c = char::from_digit(20, 10);
    ///
    /// assert_eq!(None, c);
    /// ```
    ///
    /// 传入过大的基数会导致 panic：
    ///
    /// ```should_panic
    /// // 这里会 panic。
    /// let _c = char::from_digit(1, 37);
    /// ```
    #[stable(feature = "assoc_char_funcs", since = "1.52.0")]
    #[rustc_const_stable(feature = "const_char_convert", since = "1.67.0")]
    #[must_use]
    #[inline]
    pub const fn from_digit(num: u32, radix: u32) -> Option<char> {
        super::convert::from_digit(num, radix)
    }

    /// 检查 `char` 是否是给定基数中的数字。
    ///
    /// 这里的 radix 也常称为 base：2 表示二进制，10 表示十进制，16 表示十六进制。
    /// 支持任意 2 到 36 之间的基数。
    ///
    /// 与 [`is_numeric()`] 相比，该函数只识别 `0-9`、`a-z` 和 `A-Z`。
    ///
    /// 这里的“数字”只包括以下字符：
    ///
    /// * `0-9`
    /// * `a-z`
    /// * `A-Z`
    ///
    /// 如果需要更广义的 Unicode 数字概念，见 [`is_numeric()`]。
    ///
    /// [`is_numeric()`]: #method.is_numeric
    ///
    /// # Panics
    ///
    /// 当给定基数小于 2 或大于 36 时 panic。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// assert!('1'.is_digit(10));
    /// assert!('f'.is_digit(16));
    /// assert!(!'f'.is_digit(10));
    /// ```
    ///
    /// 传入过大的基数会导致 panic：
    ///
    /// ```should_panic
    /// // 这里会 panic。
    /// '1'.is_digit(37);
    /// ```
    ///
    /// 传入过小的基数会导致 panic：
    ///
    /// ```should_panic
    /// // 这里会 panic。
    /// '1'.is_digit(1);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_char_classify", since = "1.87.0")]
    #[inline]
    pub const fn is_digit(self, radix: u32) -> bool {
        self.to_digit(radix).is_some()
    }

    /// 将 `char` 转换为给定基数中的数字值。
    ///
    /// 这里的 radix 也常称为 base：2 表示二进制，10 表示十进制，16 表示十六进制。
    /// 支持任意 2 到 36 之间的基数。
    ///
    /// 这里的“数字”只包括以下字符：
    ///
    /// * `0-9`
    /// * `a-z`
    /// * `A-Z`
    ///
    /// # 错误
    ///
    /// 如果该 `char` 不是给定基数中的一位数字，则返回 `None`。
    ///
    /// # Panics
    ///
    /// 当给定基数小于 2 或大于 36 时 panic。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// assert_eq!('1'.to_digit(10), Some(1));
    /// assert_eq!('f'.to_digit(16), Some(15));
    /// ```
    ///
    /// 传入非数字会失败：
    ///
    /// ```
    /// assert_eq!('f'.to_digit(10), None);
    /// assert_eq!('z'.to_digit(16), None);
    /// ```
    ///
    /// 传入过大的基数会导致 panic：
    ///
    /// ```should_panic
    /// // 这里会 panic。
    /// let _ = '1'.to_digit(37);
    /// ```
    /// 传入过小的基数会导致 panic：
    ///
    /// ```should_panic
    /// // 这里会 panic。
    /// let _ = '1'.to_digit(1);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_char_convert", since = "1.67.0")]
    #[rustc_diagnostic_item = "char_to_digit"]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub const fn to_digit(self, radix: u32) -> Option<u32> {
        assert!(
            radix >= 2 && radix <= 36,
            "to_digit: invalid radix -- radix must be in the range 2 to 36 inclusive"
        );
        // 检查 radix，使 radix 为已知常量时可以消除字母处理代码。
        let value = if self > '9' && radix > 10 {
            // 用掩码把 ASCII 字母转换为大写形式。
            const TO_UPPERCASE_MASK: u32 = !0b0010_0000;
            // 把 ASCII 字母转换为对应整数值：
            // A-Z => 10-35，a-z => 10-35；其他字符会产生 >= 36 的值。
            //
            // 加法溢出安全性：
            // 在减法之后应用掩码，会把第一个加数限制在永远不超过 u32::MAX - 0x20 的范围内。
            ((self as u32).wrapping_sub('A' as u32) & TO_UPPERCASE_MASK) + 10
        } else {
            // 把数字字符转换为值；非数字会环绕到大于 36 的值。
            (self as u32).wrapping_sub('0' as u32)
        };
        // FIXME(const-hack): 当 `then_some` 成为 const fn 后，改用它。
        if value < radix { Some(value) } else { None }
    }

    /// 返回一个迭代器，以 `char` 形式产生字符的十六进制 Unicode 转义序列。
    ///
    /// 转义结果采用 Rust 语法 `\u{NNNNNN}`，其中 `NNNNNN` 是十六进制表示。
    ///
    /// # 示例
    ///
    /// 作为迭代器：
    ///
    /// ```
    /// for c in '❤'.escape_unicode() {
    ///     print!("{c}");
    /// }
    /// println!();
    /// ```
    ///
    /// 直接使用 `println!`：
    ///
    /// ```
    /// println!("{}", '❤'.escape_unicode());
    /// ```
    ///
    /// 二者等价于：
    ///
    /// ```
    /// println!("\\u{{2764}}");
    /// ```
    ///
    /// 使用 [`to_string`](../std/string/trait.ToString.html#tymethod.to_string):
    ///
    /// ```
    /// assert_eq!('❤'.escape_unicode().to_string(), "\\u{2764}");
    /// ```
    #[must_use = "this returns the escaped char as an iterator, \
                  without modifying the original"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn escape_unicode(self) -> EscapeUnicode {
        EscapeUnicode::new(self)
    }

    /// `escape_debug` 的扩展版本，可按需转义 Extended Grapheme code point、
    /// 单引号和双引号。
    ///
    /// 这让字符串开头的非间距标记等字符能以更适合调试的形式展示，也允许在字符字面量中转义单引号、
    /// 在字符串中转义双引号。
    #[inline]
    pub(crate) fn escape_debug_ext(self, args: EscapeDebugExtArgs) -> EscapeDebug {
        match self {
            '\0' => EscapeDebug::backslash(ascii::Char::Digit0),
            '\t' => EscapeDebug::backslash(ascii::Char::SmallT),
            '\r' => EscapeDebug::backslash(ascii::Char::SmallR),
            '\n' => EscapeDebug::backslash(ascii::Char::SmallN),
            '\\' => EscapeDebug::backslash(ascii::Char::ReverseSolidus),
            '\"' if args.escape_double_quote => EscapeDebug::backslash(ascii::Char::QuotationMark),
            '\'' if args.escape_single_quote => EscapeDebug::backslash(ascii::Char::Apostrophe),
            _ if args.escape_grapheme_extended && self.is_grapheme_extended() => {
                EscapeDebug::unicode(self)
            }
            _ if is_printable(self) => EscapeDebug::printable(self),
            _ => EscapeDebug::unicode(self),
        }
    }

    /// 返回一个迭代器，以 `char` 形式产生字符的调试字面量转义序列。
    ///
    /// 其转义规则与 `str` 或 `char` 的 [`Debug`](core::fmt::Debug) 实现相近，
    /// 用于生成适合调试输出的文本。
    ///
    /// # 示例
    ///
    /// 作为迭代器：
    ///
    /// ```
    /// for c in '\n'.escape_debug() {
    ///     print!("{c}");
    /// }
    /// println!();
    /// ```
    ///
    /// 直接使用 `println!`：
    ///
    /// ```
    /// println!("{}", '\n'.escape_debug());
    /// ```
    ///
    /// 二者等价于：
    ///
    /// ```
    /// println!("\\n");
    /// ```
    ///
    /// 使用 [`to_string`](../std/string/trait.ToString.html#tymethod.to_string):
    ///
    /// ```
    /// assert_eq!('\n'.escape_debug().to_string(), "\\n");
    /// ```
    #[must_use = "this returns the escaped char as an iterator, \
                  without modifying the original"]
    #[stable(feature = "char_escape_debug", since = "1.20.0")]
    #[inline]
    pub fn escape_debug(self) -> EscapeDebug {
        self.escape_debug_ext(EscapeDebugExtArgs::ESCAPE_ALL)
    }

    /// 返回一个迭代器，以 `char` 形式产生字符的默认字面量转义序列。
    ///
    /// 默认规则偏向生成可被多种语言接受的字面量，包括 C++11 和类似的 C 系语言。
    /// 具体规则如下：
    ///
    /// * Tab 转义为 `\t`。
    /// * Carriage return 转义为 `\r`。
    /// * Line feed 转义为 `\n`。
    /// * 单引号转义为 `\'`。
    /// * 双引号转义为 `\"`。
    /// * 反斜杠转义为 `\\`。
    /// * `0x20` .. `0x7e`（含）范围内的 “printable ASCII” 字符不转义。
    /// * 所有其他字符使用十六进制 Unicode 转义；见 [`escape_unicode`]。
    ///
    /// [`escape_unicode`]: #method.escape_unicode
    ///
    /// # 示例
    ///
    /// 作为迭代器：
    ///
    /// ```
    /// for c in '"'.escape_default() {
    ///     print!("{c}");
    /// }
    /// println!();
    /// ```
    ///
    /// 直接使用 `println!`：
    ///
    /// ```
    /// println!("{}", '"'.escape_default());
    /// ```
    ///
    /// 二者等价于：
    ///
    /// ```
    /// println!("\\\"");
    /// ```
    ///
    /// 使用 [`to_string`](../std/string/trait.ToString.html#tymethod.to_string):
    ///
    /// ```
    /// assert_eq!('"'.escape_default().to_string(), "\\\"");
    /// ```
    #[must_use = "this returns the escaped char as an iterator, \
                  without modifying the original"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn escape_default(self) -> EscapeDefault {
        match self {
            '\t' => EscapeDefault::backslash(ascii::Char::SmallT),
            '\r' => EscapeDefault::backslash(ascii::Char::SmallR),
            '\n' => EscapeDefault::backslash(ascii::Char::SmallN),
            '\\' | '\'' | '\"' => EscapeDefault::backslash(self.as_ascii().unwrap()),
            '\x20'..='\x7e' => EscapeDefault::printable(self.as_ascii().unwrap()),
            _ => EscapeDefault::unicode(self),
        }
    }

    /// 返回该 `char` 编码为 UTF-8 时需要的字节数。
    ///
    /// 返回值始终在 1 到 4 之间（含两端），因为 `char` 是 Unicode scalar value。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let len = 'A'.len_utf8();
    /// assert_eq!(len, 1);
    ///
    /// let len = 'ß'.len_utf8();
    /// assert_eq!(len, 2);
    ///
    /// let len = 'ℝ'.len_utf8();
    /// assert_eq!(len, 3);
    ///
    /// let len = '💣'.len_utf8();
    /// assert_eq!(len, 4);
    /// ```
    ///
    /// `&str` 类型保证其内容是 UTF-8，因此可以比较每个 code point 作为 `char`
    /// 单独编码所需长度与它们在 `&str` 中实际占用长度：
    ///
    /// ```
    /// // 作为 `char`
    /// let eastern = '東';
    /// let capital = '京';
    ///
    /// // 二者都可以表示为三个字节。
    /// assert_eq!(3, eastern.len_utf8());
    /// assert_eq!(3, capital.len_utf8());
    ///
    /// // 作为 &str 时，这两个字符以 UTF-8 连续编码。
    /// let tokyo = "東京";
    ///
    /// let len = eastern.len_utf8() + capital.len_utf8();
    ///
    /// // 可以看到它们总共占六个字节……
    /// assert_eq!(6, tokyo.len());
    ///
    /// // ……与 &str 的长度一致。
    /// assert_eq!(len, tokyo.len());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_char_len_utf", since = "1.52.0")]
    #[inline]
    #[must_use]
    pub const fn len_utf8(self) -> usize {
        len_utf8(self as u32)
    }

    /// 返回该 `char` 编码为 UTF-16 时需要的 16 位 code unit 数。
    ///
    /// 对 [basic multilingual plane] 中的 Unicode scalar value，结果为 1；
    /// 对 [supplementary planes] 中的 scalar value，结果为 2。
    ///
    /// 这个概念可参见 [`len_utf8()`] 的说明；本函数是针对 UTF-16 的对应版本。
    ///
    /// [basic multilingual plane]: http://www.unicode.org/glossary/#basic_multilingual_plane
    /// [supplementary planes]: http://www.unicode.org/glossary/#supplementary_planes
    /// [`len_utf8()`]: #method.len_utf8
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let n = 'ß'.len_utf16();
    /// assert_eq!(n, 1);
    ///
    /// let len = '💣'.len_utf16();
    /// assert_eq!(len, 2);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_char_len_utf", since = "1.52.0")]
    #[inline]
    #[must_use]
    pub const fn len_utf16(self) -> usize {
        len_utf16(self as u32)
    }

    /// 将该字符以 UTF-8 编码写入给定字节缓冲区，
    /// 然后返回包含已编码字符的缓冲区子切片。
    ///
    /// # Panics
    ///
    /// 当缓冲区长度不足时 panic。长度为 4 的缓冲区足以编码任意 `char`。
    ///
    /// # 示例
    ///
    /// 在这个示例中，'ß' 需要两个字节编码。
    ///
    /// ```
    /// let mut b = [0; 2];
    ///
    /// let result = 'ß'.encode_utf8(&mut b);
    ///
    /// assert_eq!(result, "ß");
    ///
    /// assert_eq!(result.len(), 2);
    /// ```
    ///
    /// 缓冲区过小：
    ///
    /// ```should_panic
    /// let mut b = [0; 1];
    ///
    /// // 这里会 panic。
    /// 'ß'.encode_utf8(&mut b);
    /// ```
    #[stable(feature = "unicode_encode_char", since = "1.15.0")]
    #[rustc_const_stable(feature = "const_char_encode_utf8", since = "1.83.0")]
    #[inline]
    pub const fn encode_utf8(self, dst: &mut [u8]) -> &mut str {
        // SAFETY: `char` 永远不是代理项，且不超过 U+10FFFF；`encode_utf8_raw`
        // 产生的字节序列因此是合法 UTF-8，满足 `from_utf8_unchecked_mut` 前置条件。
        unsafe { from_utf8_unchecked_mut(encode_utf8_raw(self as u32, dst)) }
    }

    /// 将该字符以本机端序 UTF-16 编码写入给定 `u16` 缓冲区，
    /// 然后返回包含已编码字符的缓冲区子切片。
    ///
    /// # Panics
    ///
    /// 当缓冲区长度不足时 panic。长度为 2 的缓冲区足以编码任意 `char`。
    ///
    /// # 示例
    ///
    /// 在这个示例中，'𝕊' 需要两个 `u16` 编码。
    ///
    /// ```
    /// let mut b = [0; 2];
    ///
    /// let result = '𝕊'.encode_utf16(&mut b);
    ///
    /// assert_eq!(result.len(), 2);
    /// ```
    ///
    /// 缓冲区过小：
    ///
    /// ```should_panic
    /// let mut b = [0; 1];
    ///
    /// // 这里会 panic。
    /// '𝕊'.encode_utf16(&mut b);
    /// ```
    #[stable(feature = "unicode_encode_char", since = "1.15.0")]
    #[rustc_const_stable(feature = "const_char_encode_utf16", since = "1.84.0")]
    #[inline]
    pub const fn encode_utf16(self, dst: &mut [u16]) -> &mut [u16] {
        encode_utf16_raw(self as u32, dst)
    }

    /// 如果该 `char` 具有 `Alphabetic` 属性，则返回 `true`。
    ///
    /// `Alphabetic` 在 [Unicode Standard] 第 4 章 Character Properties 中说明，
    /// 并由 [Unicode Character Database][ucd] 的 [`DerivedCoreProperties.txt`] 指定。
    ///
    /// [Unicode Standard]: https://www.unicode.org/versions/latest/
    /// [ucd]: https://www.unicode.org/reports/tr44/
    /// [`DerivedCoreProperties.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/DerivedCoreProperties.txt
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// assert!('a'.is_alphabetic());
    /// assert!('京'.is_alphabetic());
    ///
    /// let c = '💝';
    /// // 爱有很多含义，但这个字符不具有 `Alphabetic` 属性。
    /// assert!(!c.is_alphabetic());
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn is_alphabetic(self) -> bool {
        match self {
            'a'..='z' | 'A'..='Z' => true,
            c => c > '\x7f' && unicode::Alphabetic(c),
        }
    }

    /// 如果该 `char` 具有 `Lowercase` 属性，则返回 `true`。
    ///
    /// `Lowercase` 在 [Unicode Standard] 第 4 章 Character Properties 中说明，
    /// 并由 [Unicode Character Database][ucd] 的 [`DerivedCoreProperties.txt`] 指定。
    ///
    /// [Unicode Standard]: https://www.unicode.org/versions/latest/
    /// [ucd]: https://www.unicode.org/reports/tr44/
    /// [`DerivedCoreProperties.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/DerivedCoreProperties.txt
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// assert!('a'.is_lowercase());
    /// assert!('δ'.is_lowercase());
    /// assert!(!'A'.is_lowercase());
    /// assert!(!'Δ'.is_lowercase());
    ///
    /// // 各种中文文字和标点没有大小写，因此：
    /// assert!(!'中'.is_lowercase());
    /// assert!(!' '.is_lowercase());
    /// ```
    ///
    /// 在 const 上下文中：
    ///
    /// ```
    /// const CAPITAL_DELTA_IS_LOWERCASE: bool = 'Δ'.is_lowercase();
    /// assert!(!CAPITAL_DELTA_IS_LOWERCASE);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_unicode_case_lookup", since = "1.84.0")]
    #[inline]
    pub const fn is_lowercase(self) -> bool {
        match self {
            'a'..='z' => true,
            c => c > '\x7f' && unicode::Lowercase(c),
        }
    }

    /// 如果该 `char` 具有 `Uppercase` 属性，则返回 `true`。
    ///
    /// `Uppercase` 在 [Unicode Standard] 第 4 章 Character Properties 中说明，
    /// 并由 [Unicode Character Database][ucd] 的 [`DerivedCoreProperties.txt`] 指定。
    ///
    /// [Unicode Standard]: https://www.unicode.org/versions/latest/
    /// [ucd]: https://www.unicode.org/reports/tr44/
    /// [`DerivedCoreProperties.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/DerivedCoreProperties.txt
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// assert!(!'a'.is_uppercase());
    /// assert!(!'δ'.is_uppercase());
    /// assert!('A'.is_uppercase());
    /// assert!('Δ'.is_uppercase());
    ///
    /// // 各种中文文字和标点没有大小写，因此：
    /// assert!(!'中'.is_uppercase());
    /// assert!(!' '.is_uppercase());
    /// ```
    ///
    /// 在 const 上下文中：
    ///
    /// ```
    /// const CAPITAL_DELTA_IS_UPPERCASE: bool = 'Δ'.is_uppercase();
    /// assert!(CAPITAL_DELTA_IS_UPPERCASE);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_unicode_case_lookup", since = "1.84.0")]
    #[inline]
    pub const fn is_uppercase(self) -> bool {
        match self {
            'A'..='Z' => true,
            c => c > '\x7f' && unicode::Uppercase(c),
        }
    }

    /// 如果该 `char` 具有 `White_Space` 属性，则返回 `true`。
    ///
    /// `White_Space` 由 [Unicode Character Database][ucd] 的 [`PropList.txt`] 指定。
    ///
    /// [ucd]: https://www.unicode.org/reports/tr44/
    /// [`PropList.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/PropList.txt
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// assert!(' '.is_whitespace());
    ///
    /// // 换行符。
    /// assert!('\n'.is_whitespace());
    ///
    /// // 不换行空格。
    /// assert!('\u{A0}'.is_whitespace());
    ///
    /// assert!(!'越'.is_whitespace());
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_char_classify", since = "1.87.0")]
    #[inline]
    pub const fn is_whitespace(self) -> bool {
        match self {
            ' ' | '\x09'..='\x0d' => true,
            c => c > '\x7f' && unicode::White_Space(c),
        }
    }

    /// 如果该 `char` 满足 [`is_alphabetic()`] 或 [`is_numeric()`]，则返回 `true`。
    ///
    /// ASCII 字符走快速路径；非 ASCII 字符根据 Unicode 属性表判断。
    ///
    /// [`is_alphabetic()`]: #method.is_alphabetic
    /// [`is_numeric()`]: #method.is_numeric
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// assert!('٣'.is_alphanumeric());
    /// assert!('7'.is_alphanumeric());
    /// assert!('৬'.is_alphanumeric());
    /// assert!('¾'.is_alphanumeric());
    /// assert!('①'.is_alphanumeric());
    /// assert!('K'.is_alphanumeric());
    /// assert!('و'.is_alphanumeric());
    /// assert!('藏'.is_alphanumeric());
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn is_alphanumeric(self) -> bool {
        if self.is_ascii() {
            self.is_ascii_alphanumeric()
        } else {
            unicode::Alphabetic(self) || unicode::N(self)
        }
    }

    /// 如果该 `char` 的 General Category 是控制码类别，则返回 `true`。
    ///
    /// 控制码是 General Category 为 `Cc` 的 code point；它们在 [Unicode Standard]
    /// 第 4 章 Character Properties 中说明，并由 [Unicode Character Database][ucd]
    /// 的 [`UnicodeData.txt`] 指定。
    ///
    /// [Unicode Standard]: https://www.unicode.org/versions/latest/
    /// [ucd]: https://www.unicode.org/reports/tr44/
    /// [`UnicodeData.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/UnicodeData.txt
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// // U+009C，STRING TERMINATOR。
    /// assert!(''.is_control());
    /// assert!(!'q'.is_control());
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn is_control(self) -> bool {
        // 根据 https://www.unicode.org/policies/stability_policy.html#Property_Value，
        // `Cc` 中的 code point 集合永远不会变化。
        // 因此这里可以直接硬编码匹配模式，而不必查表。
        matches!(self, '\0'..='\x1f' | '\x7f'..='\u{9f}')
    }

    /// 如果该 `char` 具有 `Grapheme_Extend` 属性，则返回 `true`。
    ///
    /// `Grapheme_Extend` 在 [Unicode Standard Annex #29 (Unicode Text
    /// Segmentation)][uax29] 中说明，并由 [Unicode Character Database][ucd]
    /// 的 [`DerivedCoreProperties.txt`] 指定。它常用于判断组合标记是否会扩展前一个字素簇。
    ///
    /// [uax29]: https://www.unicode.org/reports/tr29/
    /// [ucd]: https://www.unicode.org/reports/tr44/
    /// [`DerivedCoreProperties.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/DerivedCoreProperties.txt
    #[must_use]
    #[inline]
    pub(crate) fn is_grapheme_extended(self) -> bool {
        !self.is_ascii() && unicode::Grapheme_Extend(self)
    }

    /// 如果该 `char` 具有 `Cased` 属性，则返回 `true`。
    ///
    /// `Cased` 在 [Unicode Standard] 第 4 章 Character Properties 中说明，
    /// 并由 [Unicode Character Database][ucd] 的 [`DerivedCoreProperties.txt`] 指定。
    ///
    /// [Unicode Standard]: https://www.unicode.org/versions/latest/
    /// [ucd]: https://www.unicode.org/reports/tr44/
    /// [`DerivedCoreProperties.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/DerivedCoreProperties.txt
    #[must_use]
    #[inline]
    #[doc(hidden)]
    #[unstable(feature = "char_internals", reason = "exposed only for libstd", issue = "none")]
    pub fn is_cased(self) -> bool {
        if self.is_ascii() { self.is_ascii_alphabetic() } else { unicode::Cased(self) }
    }

    /// 如果该 `char` 具有 `Case_Ignorable` 属性，则返回 `true`。
    ///
    /// `Case_Ignorable` 在 [Unicode Standard] 第 4 章 Character Properties 中说明，
    /// 并由 [Unicode Character Database][ucd] 的 [`DerivedCoreProperties.txt`] 指定。
    ///
    /// [Unicode Standard]: https://www.unicode.org/versions/latest/
    /// [ucd]: https://www.unicode.org/reports/tr44/
    /// [`DerivedCoreProperties.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/DerivedCoreProperties.txt
    #[must_use]
    #[inline]
    #[doc(hidden)]
    #[unstable(feature = "char_internals", reason = "exposed only for libstd", issue = "none")]
    pub fn is_case_ignorable(self) -> bool {
        if self.is_ascii() {
            matches!(self, '\'' | '.' | ':' | '^' | '`')
        } else {
            unicode::Case_Ignorable(self)
        }
    }

    /// 如果该 `char` 属于 Unicode 数字相关的 General Category 之一，则返回 `true`。
    ///
    /// 数字相关的 General Category 包括 `Nd`（十进制数字）、`Nl`（类字母数字字符）
    /// 和 `No`（其他数字字符），由 [Unicode Character Database][ucd]
    /// 的 [`UnicodeData.txt`] 指定。
    ///
    /// 该方法并不覆盖所有可被人理解为数字的字符，例如表意数字 '三'。
    /// 如果需要包含用途交叠的字符，应使用暴露更细粒度字符属性的 Unicode 或语言处理库，
    /// 而不是只查看 Unicode category。
    ///
    /// 如果只想解析 ASCII 十进制数字（0-9）或 ASCII base-N 数字，请改用
    /// `is_ascii_digit` 或 `is_digit`。
    ///
    /// [Unicode Standard]: https://www.unicode.org/versions/latest/
    /// [ucd]: https://www.unicode.org/reports/tr44/
    /// [`UnicodeData.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/UnicodeData.txt
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// assert!('٣'.is_numeric());
    /// assert!('7'.is_numeric());
    /// assert!('৬'.is_numeric());
    /// assert!('¾'.is_numeric());
    /// assert!('①'.is_numeric());
    /// assert!(!'K'.is_numeric());
    /// assert!(!'و'.is_numeric());
    /// assert!(!'藏'.is_numeric());
    /// assert!(!'三'.is_numeric());
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn is_numeric(self) -> bool {
        match self {
            '0'..='9' => true,
            c => c > '\x7f' && unicode::N(c),
        }
    }

    /// 返回一个迭代器，产生该 `char` 的小写映射；结果可能是一个或多个 `char`。
    ///
    /// 如果该 `char` 没有小写映射，迭代器会产生原始 `char`。
    ///
    /// 如果 [Unicode Character Database][ucd] 的 [`UnicodeData.txt`] 给出了
    /// 一对一小写映射，迭代器会产生对应 `char`。
    ///
    /// [ucd]: https://www.unicode.org/reports/tr44/
    /// [`UnicodeData.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/UnicodeData.txt
    ///
    /// 如果该 `char` 需要特殊处理（例如映射为多个 `char`），迭代器会产生
    /// [`SpecialCasing.txt`] 给出的 `char` 序列。
    ///
    /// [`SpecialCasing.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/SpecialCasing.txt
    ///
    /// 该操作执行无条件映射，不做 tailoring；也就是说，转换不依赖上下文或语言环境。
    ///
    /// 在 [Unicode Standard] 中，第 4 章 Character Properties 讨论大小写映射，
    /// 第 3 章 Conformance 讨论默认大小写转换算法。
    ///
    /// [Unicode Standard]: https://www.unicode.org/versions/latest/
    ///
    /// # 示例
    ///
    /// 作为迭代器：
    ///
    /// ```
    /// for c in 'İ'.to_lowercase() {
    ///     print!("{c}");
    /// }
    /// println!();
    /// ```
    ///
    /// 直接使用 `println!`：
    ///
    /// ```
    /// println!("{}", 'İ'.to_lowercase());
    /// ```
    ///
    /// 二者等价于：
    ///
    /// ```
    /// println!("i\u{307}");
    /// ```
    ///
    /// 使用 [`to_string`](../std/string/trait.ToString.html#tymethod.to_string):
    ///
    /// ```
    /// assert_eq!('C'.to_lowercase().to_string(), "c");
    ///
    /// // 有时结果包含多个字符：
    /// assert_eq!('İ'.to_lowercase().to_string(), "i\u{307}");
    ///
    /// // 没有大小写对应关系的字符会转换为自身。
    /// assert_eq!('山'.to_lowercase().to_string(), "山");
    /// ```
    #[must_use = "this returns the lowercase character as a new iterator, \
                  without modifying the original"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn to_lowercase(self) -> ToLowercase {
        ToLowercase(CaseMappingIter::new(conversions::to_lower(self)))
    }

    /// 返回一个迭代器，产生该 `char` 的大写映射；结果可能是一个或多个 `char`。
    ///
    /// 如果该 `char` 没有大写映射，迭代器会产生原始 `char`。
    ///
    /// 如果 [Unicode Character Database][ucd] 的 [`UnicodeData.txt`] 给出了
    /// 一对一大写映射，迭代器会产生对应 `char`。
    ///
    /// [ucd]: https://www.unicode.org/reports/tr44/
    /// [`UnicodeData.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/UnicodeData.txt
    ///
    /// 如果该 `char` 需要特殊处理（例如映射为多个 `char`），迭代器会产生
    /// [`SpecialCasing.txt`] 给出的 `char` 序列。
    ///
    /// [`SpecialCasing.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/SpecialCasing.txt
    ///
    /// 该操作执行无条件映射，不做 tailoring；也就是说，转换不依赖上下文或语言环境。
    ///
    /// 在 [Unicode Standard] 中，第 4 章 Character Properties 讨论大小写映射，
    /// 第 3 章 Conformance 讨论默认大小写转换算法。
    ///
    /// [Unicode Standard]: https://www.unicode.org/versions/latest/
    ///
    /// # 示例
    /// `'ﬅ'`（U+FB05）是单个 Unicode code point（连字），其大写映射为 "ST"。
    ///
    /// 作为迭代器：
    ///
    /// ```
    /// for c in 'ﬅ'.to_uppercase() {
    ///     print!("{c}");
    /// }
    /// println!();
    /// ```
    ///
    /// 直接使用 `println!`：
    ///
    /// ```
    /// println!("{}", 'ﬅ'.to_uppercase());
    /// ```
    ///
    /// 二者等价于：
    ///
    /// ```
    /// println!("ST");
    /// ```
    ///
    /// 使用 [`to_string`](../std/string/trait.ToString.html#tymethod.to_string):
    ///
    /// ```
    /// assert_eq!('c'.to_uppercase().to_string(), "C");
    ///
    /// // 有时结果包含多个字符：
    /// assert_eq!('ﬅ'.to_uppercase().to_string(), "ST");
    ///
    /// // 没有大小写对应关系的字符会转换为自身。
    /// assert_eq!('山'.to_uppercase().to_string(), "山");
    /// ```
    ///
    /// # 关于区域设置的说明
    ///
    /// 在土耳其语中，拉丁字母 'i' 的对应形式不是两个，而是五个：
    ///
    /// * “无点”形式：I / ı，有时写作 ï
    /// * “有点”形式：İ / i
    ///
    /// 注意，小写有点 'i' 与普通拉丁字母相同。因此：
    ///
    /// ```
    /// let upper_i = 'i'.to_uppercase().to_string();
    /// ```
    ///
    /// 这里 `upper_i` 的值取决于文本语言：在 `en-US` 中应为 `"I"`，
    /// 在 `tr_TR` 中应为 `"İ"`。`to_uppercase()` 不考虑区域设置，因此：
    ///
    /// ```
    /// let upper_i = 'i'.to_uppercase().to_string();
    ///
    /// assert_eq!(upper_i, "I");
    /// ```
    ///
    /// 在所有语言环境中都成立。
    #[must_use = "this returns the uppercase character as a new iterator, \
                  without modifying the original"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn to_uppercase(self) -> ToUppercase {
        ToUppercase(CaseMappingIter::new(conversions::to_upper(self)))
    }

    /// 检查该值是否位于 ASCII 范围内。
    ///
    /// # 示例
    ///
    /// ```
    /// let ascii = 'a';
    /// let non_ascii = '❤';
    ///
    /// assert!(ascii.is_ascii());
    /// assert!(!non_ascii.is_ascii());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_char_is_ascii", since = "1.32.0")]
    #[rustc_diagnostic_item = "char_is_ascii"]
    #[inline]
    pub const fn is_ascii(&self) -> bool {
        *self as u32 <= 0x7F
    }

    /// 如果该值位于 ASCII 范围内，则返回 `Some`；否则返回 `None`。
    ///
    /// 当要把值继续传给接受 [`ascii::Char`] 的代码时，该方法优于先调用
    /// [`Self::is_ascii`]；返回类型已经携带 ASCII 不变量，后续代码无需再次检查。
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn as_ascii(&self) -> Option<ascii::Char> {
        if self.is_ascii() {
            // SAFETY: 上面刚检查过该值位于 ASCII 范围内。
            Some(unsafe { ascii::Char::from_u8_unchecked(*self as u8) })
        } else {
            None
        }
    }

    /// 不检查有效性，直接把该 `char` 转换为 [ASCII character](`ascii::Char`)。
    ///
    /// # 安全性(Safety）
    ///
    /// 该 `char` 必须位于 ASCII 范围内（U+0000..=U+007F）。否则会构造无效
    /// `ascii::Char`，破坏该类型“只含 ASCII”的不变量并导致 UB。
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const unsafe fn as_ascii_unchecked(&self) -> ascii::Char {
        assert_unsafe_precondition!(
            check_library_ub,
            "as_ascii_unchecked requires that the char is valid ASCII",
            (it: &char = self) => it.is_ascii()
        );

        // SAFETY: 调用方已经承诺该 `char` 位于 ASCII 范围内。
        unsafe { ascii::Char::from_u8_unchecked(*self as u8) }
    }

    /// 返回该值的 ASCII 大写等价副本。
    ///
    /// ASCII 字母 'a' 到 'z' 会映射为 'A' 到 'Z'；
    /// 非 ASCII 字母保持不变。
    ///
    /// 若要原地大写化该值，请使用 [`make_ascii_uppercase()`]。
    ///
    /// 若要同时处理非 ASCII 字符的 Unicode 大写映射，请使用 [`to_uppercase()`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let ascii = 'a';
    /// let non_ascii = '❤';
    ///
    /// assert_eq!('A', ascii.to_ascii_uppercase());
    /// assert_eq!('❤', non_ascii.to_ascii_uppercase());
    /// ```
    ///
    /// [`make_ascii_uppercase()`]: #method.make_ascii_uppercase
    /// [`to_uppercase()`]: #method.to_uppercase
    #[must_use = "to uppercase the value in-place, use `make_ascii_uppercase()`"]
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_ascii_methods_on_intrinsics", since = "1.52.0")]
    #[inline]
    pub const fn to_ascii_uppercase(&self) -> char {
        if self.is_ascii_lowercase() {
            (*self as u8).ascii_change_case_unchecked() as char
        } else {
            *self
        }
    }

    /// 返回该值的 ASCII 小写等价副本。
    ///
    /// ASCII 字母 'A' 到 'Z' 会映射为 'a' 到 'z'；
    /// 非 ASCII 字母保持不变。
    ///
    /// 若要原地小写化该值，请使用 [`make_ascii_lowercase()`]。
    ///
    /// 若要同时处理非 ASCII 字符的 Unicode 小写映射，请使用 [`to_lowercase()`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let ascii = 'A';
    /// let non_ascii = '❤';
    ///
    /// assert_eq!('a', ascii.to_ascii_lowercase());
    /// assert_eq!('❤', non_ascii.to_ascii_lowercase());
    /// ```
    ///
    /// [`make_ascii_lowercase()`]: #method.make_ascii_lowercase
    /// [`to_lowercase()`]: #method.to_lowercase
    #[must_use = "to lowercase the value in-place, use `make_ascii_lowercase()`"]
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_ascii_methods_on_intrinsics", since = "1.52.0")]
    #[inline]
    pub const fn to_ascii_lowercase(&self) -> char {
        if self.is_ascii_uppercase() {
            (*self as u8).ascii_change_case_unchecked() as char
        } else {
            *self
        }
    }

    /// 检查两个值是否按 ASCII 大小写不敏感规则匹配。
    ///
    /// 等价于 <code>[to_ascii_lowercase]\(a) == [to_ascii_lowercase]\(b)</code>。
    /// 非 ASCII 字符不会执行 Unicode 大小写折叠。
    ///
    /// # 示例
    ///
    /// ```
    /// let upper_a = 'A';
    /// let lower_a = 'a';
    /// let lower_z = 'z';
    ///
    /// assert!(upper_a.eq_ignore_ascii_case(&lower_a));
    /// assert!(upper_a.eq_ignore_ascii_case(&upper_a));
    /// assert!(!upper_a.eq_ignore_ascii_case(&lower_z));
    /// ```
    ///
    /// [to_ascii_lowercase]: #method.to_ascii_lowercase
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_ascii_methods_on_intrinsics", since = "1.52.0")]
    #[inline]
    pub const fn eq_ignore_ascii_case(&self, other: &char) -> bool {
        self.to_ascii_lowercase() == other.to_ascii_lowercase()
    }

    /// 原地转换为 ASCII 大写等价形式。
    ///
    /// ASCII 字母 'a' 到 'z' 会映射为 'A' 到 'Z'；
    /// 非 ASCII 字母保持不变。
    ///
    /// 若要返回新的大写值而不修改原值，请使用 [`to_ascii_uppercase()`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut ascii = 'a';
    ///
    /// ascii.make_ascii_uppercase();
    ///
    /// assert_eq!('A', ascii);
    /// ```
    ///
    /// [`to_ascii_uppercase()`]: #method.to_ascii_uppercase
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_make_ascii", since = "1.84.0")]
    #[inline]
    pub const fn make_ascii_uppercase(&mut self) {
        *self = self.to_ascii_uppercase();
    }

    /// 原地转换为 ASCII 小写等价形式。
    ///
    /// ASCII 字母 'A' 到 'Z' 会映射为 'a' 到 'z'；
    /// 非 ASCII 字母保持不变。
    ///
    /// 若要返回新的小写值而不修改原值，请使用 [`to_ascii_lowercase()`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut ascii = 'A';
    ///
    /// ascii.make_ascii_lowercase();
    ///
    /// assert_eq!('a', ascii);
    /// ```
    ///
    /// [`to_ascii_lowercase()`]: #method.to_ascii_lowercase
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_make_ascii", since = "1.84.0")]
    #[inline]
    pub const fn make_ascii_lowercase(&mut self) {
        *self = self.to_ascii_lowercase();
    }

    /// 检查该值是否是 ASCII 字母字符：
    ///
    /// - U+0041 'A' ..= U+005A 'Z'，或
    /// - U+0061 'a' ..= U+007A 'z'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = 'A';
    /// let uppercase_g = 'G';
    /// let a = 'a';
    /// let g = 'g';
    /// let zero = '0';
    /// let percent = '%';
    /// let space = ' ';
    /// let lf = '\n';
    /// let esc = '\x1b';
    ///
    /// assert!(uppercase_a.is_ascii_alphabetic());
    /// assert!(uppercase_g.is_ascii_alphabetic());
    /// assert!(a.is_ascii_alphabetic());
    /// assert!(g.is_ascii_alphabetic());
    /// assert!(!zero.is_ascii_alphabetic());
    /// assert!(!percent.is_ascii_alphabetic());
    /// assert!(!space.is_ascii_alphabetic());
    /// assert!(!lf.is_ascii_alphabetic());
    /// assert!(!esc.is_ascii_alphabetic());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_alphabetic(&self) -> bool {
        matches!(*self, 'A'..='Z' | 'a'..='z')
    }

    /// 检查该值是否是 ASCII 大写字符：
    /// U+0041 'A' ..= U+005A 'Z'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = 'A';
    /// let uppercase_g = 'G';
    /// let a = 'a';
    /// let g = 'g';
    /// let zero = '0';
    /// let percent = '%';
    /// let space = ' ';
    /// let lf = '\n';
    /// let esc = '\x1b';
    ///
    /// assert!(uppercase_a.is_ascii_uppercase());
    /// assert!(uppercase_g.is_ascii_uppercase());
    /// assert!(!a.is_ascii_uppercase());
    /// assert!(!g.is_ascii_uppercase());
    /// assert!(!zero.is_ascii_uppercase());
    /// assert!(!percent.is_ascii_uppercase());
    /// assert!(!space.is_ascii_uppercase());
    /// assert!(!lf.is_ascii_uppercase());
    /// assert!(!esc.is_ascii_uppercase());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_uppercase(&self) -> bool {
        matches!(*self, 'A'..='Z')
    }

    /// 检查该值是否是 ASCII 小写字符：
    /// U+0061 'a' ..= U+007A 'z'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = 'A';
    /// let uppercase_g = 'G';
    /// let a = 'a';
    /// let g = 'g';
    /// let zero = '0';
    /// let percent = '%';
    /// let space = ' ';
    /// let lf = '\n';
    /// let esc = '\x1b';
    ///
    /// assert!(!uppercase_a.is_ascii_lowercase());
    /// assert!(!uppercase_g.is_ascii_lowercase());
    /// assert!(a.is_ascii_lowercase());
    /// assert!(g.is_ascii_lowercase());
    /// assert!(!zero.is_ascii_lowercase());
    /// assert!(!percent.is_ascii_lowercase());
    /// assert!(!space.is_ascii_lowercase());
    /// assert!(!lf.is_ascii_lowercase());
    /// assert!(!esc.is_ascii_lowercase());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_lowercase(&self) -> bool {
        matches!(*self, 'a'..='z')
    }

    /// 检查该值是否是 ASCII 字母或数字字符：
    ///
    /// - U+0041 'A' ..= U+005A 'Z'，或
    /// - U+0061 'a' ..= U+007A 'z'，或
    /// - U+0030 '0' ..= U+0039 '9'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = 'A';
    /// let uppercase_g = 'G';
    /// let a = 'a';
    /// let g = 'g';
    /// let zero = '0';
    /// let percent = '%';
    /// let space = ' ';
    /// let lf = '\n';
    /// let esc = '\x1b';
    ///
    /// assert!(uppercase_a.is_ascii_alphanumeric());
    /// assert!(uppercase_g.is_ascii_alphanumeric());
    /// assert!(a.is_ascii_alphanumeric());
    /// assert!(g.is_ascii_alphanumeric());
    /// assert!(zero.is_ascii_alphanumeric());
    /// assert!(!percent.is_ascii_alphanumeric());
    /// assert!(!space.is_ascii_alphanumeric());
    /// assert!(!lf.is_ascii_alphanumeric());
    /// assert!(!esc.is_ascii_alphanumeric());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_alphanumeric(&self) -> bool {
        matches!(*self, '0'..='9') | matches!(*self, 'A'..='Z') | matches!(*self, 'a'..='z')
    }

    /// 检查该值是否是 ASCII 十进制数字：
    /// U+0030 '0' ..= U+0039 '9'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = 'A';
    /// let uppercase_g = 'G';
    /// let a = 'a';
    /// let g = 'g';
    /// let zero = '0';
    /// let percent = '%';
    /// let space = ' ';
    /// let lf = '\n';
    /// let esc = '\x1b';
    ///
    /// assert!(!uppercase_a.is_ascii_digit());
    /// assert!(!uppercase_g.is_ascii_digit());
    /// assert!(!a.is_ascii_digit());
    /// assert!(!g.is_ascii_digit());
    /// assert!(zero.is_ascii_digit());
    /// assert!(!percent.is_ascii_digit());
    /// assert!(!space.is_ascii_digit());
    /// assert!(!lf.is_ascii_digit());
    /// assert!(!esc.is_ascii_digit());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_digit(&self) -> bool {
        matches!(*self, '0'..='9')
    }

    /// 检查该值是否是 ASCII 八进制数字：
    /// U+0030 '0' ..= U+0037 '7'.
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(is_ascii_octdigit)]
    ///
    /// let uppercase_a = 'A';
    /// let a = 'a';
    /// let zero = '0';
    /// let seven = '7';
    /// let nine = '9';
    /// let percent = '%';
    /// let lf = '\n';
    ///
    /// assert!(!uppercase_a.is_ascii_octdigit());
    /// assert!(!a.is_ascii_octdigit());
    /// assert!(zero.is_ascii_octdigit());
    /// assert!(seven.is_ascii_octdigit());
    /// assert!(!nine.is_ascii_octdigit());
    /// assert!(!percent.is_ascii_octdigit());
    /// assert!(!lf.is_ascii_octdigit());
    /// ```
    #[must_use]
    #[unstable(feature = "is_ascii_octdigit", issue = "101288")]
    #[inline]
    pub const fn is_ascii_octdigit(&self) -> bool {
        matches!(*self, '0'..='7')
    }

    /// 检查该值是否是 ASCII 十六进制数字：
    ///
    /// - U+0030 '0' ..= U+0039 '9'，或
    /// - U+0041 'A' ..= U+0046 'F'，或
    /// - U+0061 'a' ..= U+0066 'f'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = 'A';
    /// let uppercase_g = 'G';
    /// let a = 'a';
    /// let g = 'g';
    /// let zero = '0';
    /// let percent = '%';
    /// let space = ' ';
    /// let lf = '\n';
    /// let esc = '\x1b';
    ///
    /// assert!(uppercase_a.is_ascii_hexdigit());
    /// assert!(!uppercase_g.is_ascii_hexdigit());
    /// assert!(a.is_ascii_hexdigit());
    /// assert!(!g.is_ascii_hexdigit());
    /// assert!(zero.is_ascii_hexdigit());
    /// assert!(!percent.is_ascii_hexdigit());
    /// assert!(!space.is_ascii_hexdigit());
    /// assert!(!lf.is_ascii_hexdigit());
    /// assert!(!esc.is_ascii_hexdigit());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_hexdigit(&self) -> bool {
        matches!(*self, '0'..='9') | matches!(*self, 'A'..='F') | matches!(*self, 'a'..='f')
    }

    /// 检查该值是否是 ASCII 标点字符：
    ///
    /// - U+0021 ..= U+002F `! " # $ % & ' ( ) * + , - . /`，或
    /// - U+003A ..= U+0040 `: ; < = > ? @`，或
    /// - U+005B ..= U+0060 ``[ \ ] ^ _ ` ``，或
    /// - U+007B ..= U+007E `{ | } ~`
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = 'A';
    /// let uppercase_g = 'G';
    /// let a = 'a';
    /// let g = 'g';
    /// let zero = '0';
    /// let percent = '%';
    /// let space = ' ';
    /// let lf = '\n';
    /// let esc = '\x1b';
    ///
    /// assert!(!uppercase_a.is_ascii_punctuation());
    /// assert!(!uppercase_g.is_ascii_punctuation());
    /// assert!(!a.is_ascii_punctuation());
    /// assert!(!g.is_ascii_punctuation());
    /// assert!(!zero.is_ascii_punctuation());
    /// assert!(percent.is_ascii_punctuation());
    /// assert!(!space.is_ascii_punctuation());
    /// assert!(!lf.is_ascii_punctuation());
    /// assert!(!esc.is_ascii_punctuation());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_punctuation(&self) -> bool {
        matches!(*self, '!'..='/')
            | matches!(*self, ':'..='@')
            | matches!(*self, '['..='`')
            | matches!(*self, '{'..='~')
    }

    /// 检查该值是否是 ASCII 图形字符：
    /// U+0021 '!' ..= U+007E '~'.
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = 'A';
    /// let uppercase_g = 'G';
    /// let a = 'a';
    /// let g = 'g';
    /// let zero = '0';
    /// let percent = '%';
    /// let space = ' ';
    /// let lf = '\n';
    /// let esc = '\x1b';
    ///
    /// assert!(uppercase_a.is_ascii_graphic());
    /// assert!(uppercase_g.is_ascii_graphic());
    /// assert!(a.is_ascii_graphic());
    /// assert!(g.is_ascii_graphic());
    /// assert!(zero.is_ascii_graphic());
    /// assert!(percent.is_ascii_graphic());
    /// assert!(!space.is_ascii_graphic());
    /// assert!(!lf.is_ascii_graphic());
    /// assert!(!esc.is_ascii_graphic());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_graphic(&self) -> bool {
        matches!(*self, '!'..='~')
    }

    /// 检查该值是否是 ASCII whitespace 字符：
    /// U+0020 SPACE, U+0009 HORIZONTAL TAB, U+000A LINE FEED,
    /// U+000C FORM FEED, or U+000D CARRIAGE RETURN.
    ///
    /// Rust 使用 WhatWG Infra Standard 对 [ASCII whitespace][infra-aw] 的定义。
    /// 其他上下文中还广泛使用若干不同定义。例如，[POSIX locale][pct] 除了上述字符外还包含
    /// U+000B VERTICAL TAB；但同一规范中 Bourne shell 的
    /// ["field splitting" 默认规则][bfs] 只把 SPACE、HORIZONTAL TAB 和 LINE FEED
    /// 视为 whitespace。
    ///
    /// 如果程序要处理既有文件格式，在使用该函数前应先确认该格式采用哪一种 whitespace 定义。
    ///
    /// [infra-aw]: https://infra.spec.whatwg.org/#ascii-whitespace
    /// [pct]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap07.html#tag_07_03_01
    /// [bfs]: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html#tag_18_06_05
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = 'A';
    /// let uppercase_g = 'G';
    /// let a = 'a';
    /// let g = 'g';
    /// let zero = '0';
    /// let percent = '%';
    /// let space = ' ';
    /// let lf = '\n';
    /// let esc = '\x1b';
    ///
    /// assert!(!uppercase_a.is_ascii_whitespace());
    /// assert!(!uppercase_g.is_ascii_whitespace());
    /// assert!(!a.is_ascii_whitespace());
    /// assert!(!g.is_ascii_whitespace());
    /// assert!(!zero.is_ascii_whitespace());
    /// assert!(!percent.is_ascii_whitespace());
    /// assert!(space.is_ascii_whitespace());
    /// assert!(lf.is_ascii_whitespace());
    /// assert!(!esc.is_ascii_whitespace());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_whitespace(&self) -> bool {
        matches!(*self, '\t' | '\n' | '\x0C' | '\r' | ' ')
    }

    /// 检查该值是否是 ASCII 控制字符：
    /// U+0000 NUL ..= U+001F UNIT SEPARATOR, or U+007F DELETE.
    /// 注意，大多数 ASCII whitespace 字符也是控制字符，但 SPACE 不是。
    ///
    /// # 示例
    ///
    /// ```
    /// let uppercase_a = 'A';
    /// let uppercase_g = 'G';
    /// let a = 'a';
    /// let g = 'g';
    /// let zero = '0';
    /// let percent = '%';
    /// let space = ' ';
    /// let lf = '\n';
    /// let esc = '\x1b';
    ///
    /// assert!(!uppercase_a.is_ascii_control());
    /// assert!(!uppercase_g.is_ascii_control());
    /// assert!(!a.is_ascii_control());
    /// assert!(!g.is_ascii_control());
    /// assert!(!zero.is_ascii_control());
    /// assert!(!percent.is_ascii_control());
    /// assert!(!space.is_ascii_control());
    /// assert!(lf.is_ascii_control());
    /// assert!(esc.is_ascii_control());
    /// ```
    #[must_use]
    #[stable(feature = "ascii_ctype_on_intrinsics", since = "1.24.0")]
    #[rustc_const_stable(feature = "const_ascii_ctype_on_intrinsics", since = "1.47.0")]
    #[inline]
    pub const fn is_ascii_control(&self) -> bool {
        matches!(*self, '\0'..='\x1F' | '\x7F')
    }
}

pub(crate) struct EscapeDebugExtArgs {
    /// 是否转义 Extended Grapheme code point？
    pub(crate) escape_grapheme_extended: bool,

    /// 是否转义单引号？
    pub(crate) escape_single_quote: bool,

    /// 是否转义双引号？
    pub(crate) escape_double_quote: bool,
}

impl EscapeDebugExtArgs {
    pub(crate) const ESCAPE_ALL: Self = Self {
        escape_grapheme_extended: true,
        escape_single_quote: true,
        escape_double_quote: true,
    };
}

#[inline]
#[must_use]
const fn len_utf8(code: u32) -> usize {
    match code {
        ..MAX_ONE_B => 1,
        ..MAX_TWO_B => 2,
        ..MAX_THREE_B => 3,
        _ => 4,
    }
}

#[inline]
#[must_use]
const fn len_utf16(code: u32) -> usize {
    if (code & 0xFFFF) == code { 1 } else { 2 }
}

/// 将原始 `u32` 值按 UTF-8 形式编码到给定字节缓冲区，
/// 然后返回包含已编码内容的缓冲区子切片。
///
/// 与 `char::encode_utf8` 不同，该方法也处理代理项范围内的 code point。
/// （创建位于代理项范围内的 `char` 是 UB。）结果是合法 [generalized UTF-8]，
/// 但不是合法 UTF-8；这类路径供 WTF-8/OS 字符串桥接等内部表示使用。
///
/// [generalized UTF-8]: https://simonsapin.github.io/wtf-8/#generalized-utf8
///
/// # Panics
///
/// 当缓冲区长度不足时 panic。长度为 4 的缓冲区足以编码任意 `char` 或 generalized UTF-8 code point。
#[unstable(feature = "char_internals", reason = "exposed only for libstd", issue = "none")]
#[doc(hidden)]
#[inline]
pub const fn encode_utf8_raw(code: u32, dst: &mut [u8]) -> &mut [u8] {
    let len = len_utf8(code);
    if dst.len() < len {
        const_panic!(
            "encode_utf8: buffer does not have enough bytes to encode code point",
            "encode_utf8: need {len} bytes to encode U+{code:04X} but buffer has just {dst_len}",
            code: u32 = code,
            len: usize = len,
            dst_len: usize = dst.len(),
        );
    }

    // SAFETY: 已检查 `dst` 至少具有编码该 code point 所需的 `len` 字节。
    unsafe { encode_utf8_raw_unchecked(code, dst.as_mut_ptr()) };

    // SAFETY: `<&mut [u8]>::as_mut_ptr` 保证返回有效指针，且 `len` 已经检查在切片范围内。
    unsafe { slice::from_raw_parts_mut(dst.as_mut_ptr(), len) }
}

/// 将原始 `u32` 值按 UTF-8 形式编码到 `dst` 指向的字节缓冲区。
///
/// 与 `char::encode_utf8` 不同，该方法也处理代理项范围内的 code point。
/// （创建位于代理项范围内的 `char` 是 UB。）结果是合法 [generalized UTF-8]，
/// 但不是合法 UTF-8。
///
/// [generalized UTF-8]: https://simonsapin.github.io/wtf-8/#generalized-utf8
///
/// # 安全性(Safety）
///
/// 调用方必须保证 `dst` 指向一段可写、对 `u8` 有效的缓冲区，且至少包含
/// `len_utf8(code)` 个字节。若缓冲区不足、指针无效或写入越界，行为未定义。
/// 长度为 4 的缓冲区足以编码任意 `char`，也足以覆盖 generalized UTF-8 中的代理项 code point。
///
/// 安全版本见 [`encode_utf8_raw`]。
#[unstable(feature = "char_internals", reason = "exposed only for libstd", issue = "none")]
#[doc(hidden)]
#[inline]
pub const unsafe fn encode_utf8_raw_unchecked(code: u32, dst: *mut u8) {
    let len = len_utf8(code);
    // SAFETY: 调用方必须保证 `dst` 指向的缓冲区至少有 `len` 个可写字节；
    // 本函数只在该范围内写入编码结果。
    unsafe {
        if len == 1 {
            *dst = code as u8;
            return;
        }

        let last1 = (code >> 0 & 0x3F) as u8 | TAG_CONT;
        let last2 = (code >> 6 & 0x3F) as u8 | TAG_CONT;
        let last3 = (code >> 12 & 0x3F) as u8 | TAG_CONT;
        let last4 = (code >> 18 & 0x3F) as u8 | TAG_FOUR_B;

        if len == 2 {
            *dst = last2 | TAG_TWO_B;
            *dst.add(1) = last1;
            return;
        }

        if len == 3 {
            *dst = last3 | TAG_THREE_B;
            *dst.add(1) = last2;
            *dst.add(2) = last1;
            return;
        }

        *dst = last4;
        *dst.add(1) = last3;
        *dst.add(2) = last2;
        *dst.add(3) = last1;
    }
}

/// 将原始 `u32` 值按本机端序 UTF-16 编码到给定 `u16` 缓冲区，
/// 然后返回包含已编码内容的缓冲区子切片。
///
/// 与 `char::encode_utf16` 不同，该方法也处理代理项范围内的 code point。
/// （创建位于代理项范围内的 `char` 是 UB。）该内部函数用于需要保留可能非良构
/// UTF-16 code unit 的桥接逻辑。
///
/// # Panics
///
/// 当缓冲区长度不足时 panic。长度为 2 的缓冲区足以编码任意 `char`。
#[unstable(feature = "char_internals", reason = "exposed only for libstd", issue = "none")]
#[doc(hidden)]
#[inline]
pub const fn encode_utf16_raw(mut code: u32, dst: &mut [u16]) -> &mut [u16] {
    let len = len_utf16(code);
    match (len, &mut *dst) {
        (1, [a, ..]) => {
            *a = code as u16;
        }
        (2, [a, b, ..]) => {
            code -= 0x1_0000;
            *a = (code >> 10) as u16 | 0xD800;
            *b = (code & 0x3FF) as u16 | 0xDC00;
        }
        _ => {
            const_panic!(
                "encode_utf16: buffer does not have enough bytes to encode code point",
                "encode_utf16: need {len} bytes to encode U+{code:04X} but buffer has just {dst_len}",
                code: u32 = code,
                len: usize = len,
                dst_len: usize = dst.len(),
            )
        }
    };
    // SAFETY: `<&mut [u16]>::as_mut_ptr` 保证返回有效指针，且 `len` 已经检查在切片范围内。
    unsafe { slice::from_raw_parts_mut(dst.as_mut_ptr(), len) }
}
