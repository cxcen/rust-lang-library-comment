//! 本文件内部使用 `AsciiChar` 这个名字,即使当前对外并不以该名称暴露。这样做可以避免
//! 这里稍有拼写差错时 rustc 给出大量“你是不是想写 `char`?”的建议;同时,文件中也确实会
//! 有意提到普通 `char`,区分两个名字能让语义更清楚。

use crate::mem::transmute;
use crate::{assert_unsafe_precondition, fmt};

/// U+0000 到 U+007F 这 128 个 Unicode 字符之一,通常称为 [ASCII] 子集。
///
/// 严格来说,这是 Unicode 的第一个[区块][block]:_Basic Latin_。详见
/// [*C0 Controls and Basic Latin*][chart] 码表。
///
/// 该区块源自更早的 7-bit 字符编码标准,例如 ANSI X3.4-1977、ISO 646-1973
/// 以及 [NIST FIPS 1-2]。
///
/// # 何时使用
///
/// 这个子集的主要优势是:它始终是合法 UTF-8。因此 `&[ascii::Char]` -> `&str`
/// 以及相关转换都是 O(1):完全不需要运行时检查。
///
/// 如果代码是在消费字符串,通常应处理完整 Unicode,也就是接受 `str`,而不是把输入限制为
/// `ascii::Char`。
///
/// 不过,某些格式为了保持 8-bit-clean,会有意只产生 ASCII 输出。在这些场景下,
/// 生成 `ascii::Char` 往往比处理通用 UTF-8 字符串的变长性质更简单、更快;同时结果仍可
/// 通过 `str` 相关 API 与 Rust 的普通字符串生态配合使用。
///
/// 例如,UUID 库可以把 UUID 的字符串表示生成为 `[ascii::Char; 36]`,从而避免内存分配;
/// 调用方仍能通过 `as_str` 把它当作 UTF-8 使用,无需像 `[u8; 36]` 那样为校验付费,
/// 也不需要写 `unsafe` 代码。
///
/// # 布局
///
/// 本类型保证大小与对齐均为 1 字节。
///
/// # 名称
///
/// 本类型的变体名来自字符的 [Unicode 名称][NamesList],转为大驼峰后做了少量调整:
/// - 对 `<control>` 字符,使用其主要别名。
/// - 去掉 `LATIN`,因为该区块没有非拉丁字母。
/// - 去掉 `LETTER`,因为本区块中 `CAPITAL`/`SMALL` 已足以区分。
/// - `DIGIT` 使用单个数字,而不是写成 `ZERO`、`ONE` 等。
///
/// [ASCII]: https://www.unicode.org/glossary/index.html#ASCII
/// [block]: https://www.unicode.org/glossary/index.html#block
/// [chart]: https://www.unicode.org/charts/PDF/U0000.pdf
/// [NIST FIPS 1-2]: https://nvlpubs.nist.gov/nistpubs/Legacy/FIPS/fipspub1-2-1977.pdf
/// [NamesList]: https://www.unicode.org/Public/15.0.0/ucd/NamesList.txt
#[derive(Copy, Hash)]
#[derive_const(Clone, Eq, PartialEq, Ord, PartialOrd)]
#[unstable(feature = "ascii_char", issue = "110998")]
#[repr(u8)]
pub enum AsciiChar {
    /// U+0000(默认变体)
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Null = 0,
    /// U+0001
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    StartOfHeading = 1,
    /// U+0002
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    StartOfText = 2,
    /// U+0003
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    EndOfText = 3,
    /// U+0004
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    EndOfTransmission = 4,
    /// U+0005
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Enquiry = 5,
    /// U+0006
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Acknowledge = 6,
    /// U+0007
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Bell = 7,
    /// U+0008
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Backspace = 8,
    /// U+0009
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CharacterTabulation = 9,
    /// U+000A
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    LineFeed = 10,
    /// U+000B
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    LineTabulation = 11,
    /// U+000C
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    FormFeed = 12,
    /// U+000D
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CarriageReturn = 13,
    /// U+000E
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    ShiftOut = 14,
    /// U+000F
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    ShiftIn = 15,
    /// U+0010
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    DataLinkEscape = 16,
    /// U+0011
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    DeviceControlOne = 17,
    /// U+0012
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    DeviceControlTwo = 18,
    /// U+0013
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    DeviceControlThree = 19,
    /// U+0014
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    DeviceControlFour = 20,
    /// U+0015
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    NegativeAcknowledge = 21,
    /// U+0016
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SynchronousIdle = 22,
    /// U+0017
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    EndOfTransmissionBlock = 23,
    /// U+0018
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Cancel = 24,
    /// U+0019
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    EndOfMedium = 25,
    /// U+001A
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Substitute = 26,
    /// U+001B
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Escape = 27,
    /// U+001C
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    InformationSeparatorFour = 28,
    /// U+001D
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    InformationSeparatorThree = 29,
    /// U+001E
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    InformationSeparatorTwo = 30,
    /// U+001F
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    InformationSeparatorOne = 31,
    /// U+0020
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Space = 32,
    /// U+0021
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    ExclamationMark = 33,
    /// U+0022
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    QuotationMark = 34,
    /// U+0023
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    NumberSign = 35,
    /// U+0024
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    DollarSign = 36,
    /// U+0025
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    PercentSign = 37,
    /// U+0026
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Ampersand = 38,
    /// U+0027
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Apostrophe = 39,
    /// U+0028
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    LeftParenthesis = 40,
    /// U+0029
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    RightParenthesis = 41,
    /// U+002A
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Asterisk = 42,
    /// U+002B
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    PlusSign = 43,
    /// U+002C
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Comma = 44,
    /// U+002D
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    HyphenMinus = 45,
    /// U+002E
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    FullStop = 46,
    /// U+002F
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Solidus = 47,
    /// U+0030
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Digit0 = 48,
    /// U+0031
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Digit1 = 49,
    /// U+0032
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Digit2 = 50,
    /// U+0033
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Digit3 = 51,
    /// U+0034
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Digit4 = 52,
    /// U+0035
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Digit5 = 53,
    /// U+0036
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Digit6 = 54,
    /// U+0037
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Digit7 = 55,
    /// U+0038
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Digit8 = 56,
    /// U+0039
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Digit9 = 57,
    /// U+003A
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Colon = 58,
    /// U+003B
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Semicolon = 59,
    /// U+003C
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    LessThanSign = 60,
    /// U+003D
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    EqualsSign = 61,
    /// U+003E
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    GreaterThanSign = 62,
    /// U+003F
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    QuestionMark = 63,
    /// U+0040
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CommercialAt = 64,
    /// U+0041
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalA = 65,
    /// U+0042
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalB = 66,
    /// U+0043
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalC = 67,
    /// U+0044
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalD = 68,
    /// U+0045
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalE = 69,
    /// U+0046
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalF = 70,
    /// U+0047
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalG = 71,
    /// U+0048
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalH = 72,
    /// U+0049
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalI = 73,
    /// U+004A
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalJ = 74,
    /// U+004B
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalK = 75,
    /// U+004C
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalL = 76,
    /// U+004D
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalM = 77,
    /// U+004E
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalN = 78,
    /// U+004F
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalO = 79,
    /// U+0050
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalP = 80,
    /// U+0051
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalQ = 81,
    /// U+0052
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalR = 82,
    /// U+0053
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalS = 83,
    /// U+0054
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalT = 84,
    /// U+0055
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalU = 85,
    /// U+0056
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalV = 86,
    /// U+0057
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalW = 87,
    /// U+0058
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalX = 88,
    /// U+0059
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalY = 89,
    /// U+005A
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CapitalZ = 90,
    /// U+005B
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    LeftSquareBracket = 91,
    /// U+005C
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    ReverseSolidus = 92,
    /// U+005D
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    RightSquareBracket = 93,
    /// U+005E
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    CircumflexAccent = 94,
    /// U+005F
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    LowLine = 95,
    /// U+0060
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    GraveAccent = 96,
    /// U+0061
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallA = 97,
    /// U+0062
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallB = 98,
    /// U+0063
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallC = 99,
    /// U+0064
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallD = 100,
    /// U+0065
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallE = 101,
    /// U+0066
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallF = 102,
    /// U+0067
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallG = 103,
    /// U+0068
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallH = 104,
    /// U+0069
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallI = 105,
    /// U+006A
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallJ = 106,
    /// U+006B
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallK = 107,
    /// U+006C
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallL = 108,
    /// U+006D
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallM = 109,
    /// U+006E
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallN = 110,
    /// U+006F
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallO = 111,
    /// U+0070
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallP = 112,
    /// U+0071
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallQ = 113,
    /// U+0072
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallR = 114,
    /// U+0073
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallS = 115,
    /// U+0074
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallT = 116,
    /// U+0075
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallU = 117,
    /// U+0076
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallV = 118,
    /// U+0077
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallW = 119,
    /// U+0078
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallX = 120,
    /// U+0079
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallY = 121,
    /// U+007A
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    SmallZ = 122,
    /// U+007B
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    LeftCurlyBracket = 123,
    /// U+007C
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    VerticalLine = 124,
    /// U+007D
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    RightCurlyBracket = 125,
    /// U+007E
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Tilde = 126,
    /// U+007F
    #[unstable(feature = "ascii_char_variants", issue = "110998")]
    Delete = 127,
}

impl AsciiChar {
    /// ASCII 码位最小的字符。
    #[unstable(feature = "ascii_char", issue = "110998")]
    pub const MIN: Self = Self::Null;

    /// ASCII 码位最大的字符。
    #[unstable(feature = "ascii_char", issue = "110998")]
    pub const MAX: Self = Self::Delete;

    /// 由字节 `b` 创建 ASCII 字符;若 `b` 超出 ASCII 范围则返回 `None`。
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn from_u8(b: u8) -> Option<Self> {
        if b <= 127 {
            // SAFETY: 上面刚检查过 `b` 位于 ASCII 范围内。
            Some(unsafe { Self::from_u8_unchecked(b) })
        } else {
            None
        }
    }

    /// 由字节 `b` 创建 ASCII 字符,但不检查它是否有效。
    ///
    /// # 安全性(Safety）
    ///
    /// `b` 必须位于 `0..=127`;否则 `transmute` 会产生不属于 `AsciiChar` 枚举的值,
    /// 立即导致未定义行为。
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const unsafe fn from_u8_unchecked(b: u8) -> Self {
        // SAFETY: 本函数的安全前置条件保证 `b` 位于 ASCII 范围内。
        unsafe { transmute(b) }
    }

    /// 传入*数字* `0`、`1`、…、`9` 时,分别返回*字符* `'0'`、`'1'`、…、`'9'`。
    ///
    /// 若 `d >= 10`,返回 `None`。
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn digit(d: u8) -> Option<Self> {
        if d < 10 {
            // SAFETY: 上面刚检查过它位于十进制数字范围内。
            Some(unsafe { Self::digit_unchecked(d) })
        } else {
            None
        }
    }

    /// 传入*数字* `0`、`1`、…、`9` 时,分别返回*字符* `'0'`、`'1'`、…、`'9'`,
    /// 但不检查参数是否在范围内。
    ///
    /// # 安全性(Safety）
    ///
    /// 若以 `d > 64` 调用,本函数会立即导致 UB。
    ///
    /// 若 `d >= 10` 且 `d <= 64`,本函数允许返回任意值或 panic。尤其不能期望它返回
    /// 十六进制数字,也不能把它当作十进制数字的某种合理扩展。
    ///
    /// (这个较宽松的安全条件是为了让使用本方法的代码更容易证明健全性:实现并不需要更精确的
    /// 条件。它不是为了让其他参数产生有用结果。稳定化前该条件可能会收紧。)
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    #[track_caller]
    pub const unsafe fn digit_unchecked(d: u8) -> Self {
        assert_unsafe_precondition!(
            check_library_ub,
            "`ascii::Char::digit_unchecked` input cannot exceed 9.",
            (d: u8 = d) => d < 10
        );

        // SAFETY: `'0'` 到 `'9'` 是 U+0030 到 U+0039。由于 `d` 必须不超过 64,
        // 加法结果最大为 112(0x70),既不会溢出,也仍位于 ASCII 范围内。
        unsafe {
            let byte = b'0'.unchecked_add(d);
            Self::from_u8_unchecked(byte)
        }
    }

    /// 以字节形式取得这个 ASCII 字符。
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// 以 `char` Unicode 标量值形式取得这个 ASCII 字符。
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn to_char(self) -> char {
        self as u8 as char
    }

    /// 把这个 ASCII 字符视为只含一个 UTF-8 code unit 的 `str`。
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn as_str(&self) -> &str {
        crate::slice::from_ref(self).as_str()
    }

    /// 返回本值的大写等价字符副本。
    ///
    /// 字母 'a' 到 'z' 会映射到 'A' 到 'Z'。
    ///
    /// 若要原地转为大写,使用 [`make_uppercase`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let lowercase_a = ascii::Char::SmallA;
    ///
    /// assert_eq!(
    ///     ascii::Char::CapitalA,
    ///     lowercase_a.to_uppercase(),
    /// );
    /// ```
    ///
    /// [`make_uppercase`]: Self::make_uppercase
    #[must_use = "to uppercase the value in-place, use `make_uppercase()`"]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn to_uppercase(self) -> Self {
        let uppercase_byte = self.to_u8().to_ascii_uppercase();
        // SAFETY: 翻转第 6 位不会把 ASCII 转成非 ASCII。
        unsafe { Self::from_u8_unchecked(uppercase_byte) }
    }

    /// 返回本值的小写等价字符副本。
    ///
    /// 字母 'A' 到 'Z' 会映射到 'a' 到 'z'。
    ///
    /// 若要原地转为小写,使用 [`make_lowercase`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let uppercase_a = ascii::Char::CapitalA;
    ///
    /// assert_eq!(
    ///     ascii::Char::SmallA,
    ///     uppercase_a.to_lowercase(),
    /// );
    /// ```
    ///
    /// [`make_lowercase`]: Self::make_lowercase
    #[must_use = "to lowercase the value in-place, use `make_lowercase()`"]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn to_lowercase(self) -> Self {
        let lowercase_byte = self.to_u8().to_ascii_lowercase();
        // SAFETY: 设置第 6 位不会把 ASCII 转成非 ASCII。
        unsafe { Self::from_u8_unchecked(lowercase_byte) }
    }

    /// 检查两个值在忽略 ASCII 大小写后是否匹配。
    ///
    /// 等价于 `to_lowercase(a) == to_lowercase(b)`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let lowercase_a = ascii::Char::SmallA;
    /// let uppercase_a = ascii::Char::CapitalA;
    ///
    /// assert!(lowercase_a.eq_ignore_case(uppercase_a));
    /// ```
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn eq_ignore_case(self, other: Self) -> bool {
        // FIXME(const-hack): 一旦 `Self` 的 `PartialEq` 可在 const 中使用,
        // 就把 `arg.to_u8().to_ascii_lowercase()` 改成 `arg.to_lowercase()`。
        self.to_u8().to_ascii_lowercase() == other.to_u8().to_ascii_lowercase()
    }

    /// 将本值原地转换为其大写等价字符。
    ///
    /// 字母 'a' 到 'z' 会映射到 'A' 到 'Z'。
    ///
    /// 若要返回新的大写值而不修改现有值,使用 [`to_uppercase`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let mut letter_a = ascii::Char::SmallA;
    ///
    /// letter_a.make_uppercase();
    ///
    /// assert_eq!(ascii::Char::CapitalA, letter_a);
    /// ```
    ///
    /// [`to_uppercase`]: Self::to_uppercase
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn make_uppercase(&mut self) {
        *self = self.to_uppercase();
    }

    /// 将本值原地转换为其小写等价字符。
    ///
    /// 字母 'A' 到 'Z' 会映射到 'a' 到 'z'。
    ///
    /// 若要返回新的小写值而不修改现有值,使用 [`to_lowercase`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let mut letter_a = ascii::Char::CapitalA;
    ///
    /// letter_a.make_lowercase();
    ///
    /// assert_eq!(ascii::Char::SmallA, letter_a);
    /// ```
    ///
    /// [`to_lowercase`]: Self::to_lowercase
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn make_lowercase(&mut self) {
        *self = self.to_lowercase();
    }

    /// 检查本值是否是 ASCII 字母:
    ///
    /// - 0x41 'A' ..= 0x5A 'Z', or
    /// - 0x61 'a' ..= 0x7A 'z'.
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let uppercase_a = ascii::Char::CapitalA;
    /// let uppercase_g = ascii::Char::CapitalG;
    /// let a = ascii::Char::SmallA;
    /// let g = ascii::Char::SmallG;
    /// let zero = ascii::Char::Digit0;
    /// let percent = ascii::Char::PercentSign;
    /// let space = ascii::Char::Space;
    /// let lf = ascii::Char::LineFeed;
    /// let esc = ascii::Char::Escape;
    ///
    /// assert!(uppercase_a.is_alphabetic());
    /// assert!(uppercase_g.is_alphabetic());
    /// assert!(a.is_alphabetic());
    /// assert!(g.is_alphabetic());
    /// assert!(!zero.is_alphabetic());
    /// assert!(!percent.is_alphabetic());
    /// assert!(!space.is_alphabetic());
    /// assert!(!lf.is_alphabetic());
    /// assert!(!esc.is_alphabetic());
    /// ```
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn is_alphabetic(self) -> bool {
        self.to_u8().is_ascii_alphabetic()
    }

    /// 检查本值是否是 ASCII 大写字母:
    /// 0x41 'A' ..= 0x5A 'Z'.
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let uppercase_a = ascii::Char::CapitalA;
    /// let uppercase_g = ascii::Char::CapitalG;
    /// let a = ascii::Char::SmallA;
    /// let g = ascii::Char::SmallG;
    /// let zero = ascii::Char::Digit0;
    /// let percent = ascii::Char::PercentSign;
    /// let space = ascii::Char::Space;
    /// let lf = ascii::Char::LineFeed;
    /// let esc = ascii::Char::Escape;
    ///
    /// assert!(uppercase_a.is_uppercase());
    /// assert!(uppercase_g.is_uppercase());
    /// assert!(!a.is_uppercase());
    /// assert!(!g.is_uppercase());
    /// assert!(!zero.is_uppercase());
    /// assert!(!percent.is_uppercase());
    /// assert!(!space.is_uppercase());
    /// assert!(!lf.is_uppercase());
    /// assert!(!esc.is_uppercase());
    /// ```
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn is_uppercase(self) -> bool {
        self.to_u8().is_ascii_uppercase()
    }

    /// 检查本值是否是 ASCII 小写字母:
    /// 0x61 'a' ..= 0x7A 'z'.
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let uppercase_a = ascii::Char::CapitalA;
    /// let uppercase_g = ascii::Char::CapitalG;
    /// let a = ascii::Char::SmallA;
    /// let g = ascii::Char::SmallG;
    /// let zero = ascii::Char::Digit0;
    /// let percent = ascii::Char::PercentSign;
    /// let space = ascii::Char::Space;
    /// let lf = ascii::Char::LineFeed;
    /// let esc = ascii::Char::Escape;
    ///
    /// assert!(!uppercase_a.is_lowercase());
    /// assert!(!uppercase_g.is_lowercase());
    /// assert!(a.is_lowercase());
    /// assert!(g.is_lowercase());
    /// assert!(!zero.is_lowercase());
    /// assert!(!percent.is_lowercase());
    /// assert!(!space.is_lowercase());
    /// assert!(!lf.is_lowercase());
    /// assert!(!esc.is_lowercase());
    /// ```
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn is_lowercase(self) -> bool {
        self.to_u8().is_ascii_lowercase()
    }

    /// 检查本值是否是 ASCII 字母或数字:
    ///
    /// - 0x41 'A' ..= 0x5A 'Z', or
    /// - 0x61 'a' ..= 0x7A 'z', or
    /// - 0x30 '0' ..= 0x39 '9'.
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let uppercase_a = ascii::Char::CapitalA;
    /// let uppercase_g = ascii::Char::CapitalG;
    /// let a = ascii::Char::SmallA;
    /// let g = ascii::Char::SmallG;
    /// let zero = ascii::Char::Digit0;
    /// let percent = ascii::Char::PercentSign;
    /// let space = ascii::Char::Space;
    /// let lf = ascii::Char::LineFeed;
    /// let esc = ascii::Char::Escape;
    ///
    /// assert!(uppercase_a.is_alphanumeric());
    /// assert!(uppercase_g.is_alphanumeric());
    /// assert!(a.is_alphanumeric());
    /// assert!(g.is_alphanumeric());
    /// assert!(zero.is_alphanumeric());
    /// assert!(!percent.is_alphanumeric());
    /// assert!(!space.is_alphanumeric());
    /// assert!(!lf.is_alphanumeric());
    /// assert!(!esc.is_alphanumeric());
    /// ```
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn is_alphanumeric(self) -> bool {
        self.to_u8().is_ascii_alphanumeric()
    }

    /// 检查本值是否是 ASCII 十进制数字:
    /// 0x30 '0' ..= 0x39 '9'.
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let uppercase_a = ascii::Char::CapitalA;
    /// let uppercase_g = ascii::Char::CapitalG;
    /// let a = ascii::Char::SmallA;
    /// let g = ascii::Char::SmallG;
    /// let zero = ascii::Char::Digit0;
    /// let percent = ascii::Char::PercentSign;
    /// let space = ascii::Char::Space;
    /// let lf = ascii::Char::LineFeed;
    /// let esc = ascii::Char::Escape;
    ///
    /// assert!(!uppercase_a.is_digit());
    /// assert!(!uppercase_g.is_digit());
    /// assert!(!a.is_digit());
    /// assert!(!g.is_digit());
    /// assert!(zero.is_digit());
    /// assert!(!percent.is_digit());
    /// assert!(!space.is_digit());
    /// assert!(!lf.is_digit());
    /// assert!(!esc.is_digit());
    /// ```
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn is_digit(self) -> bool {
        self.to_u8().is_ascii_digit()
    }

    /// 检查本值是否是 ASCII 八进制数字:
    /// 0x30 '0' ..= 0x37 '7'.
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants, is_ascii_octdigit)]
    ///
    /// use std::ascii;
    ///
    /// let uppercase_a = ascii::Char::CapitalA;
    /// let a = ascii::Char::SmallA;
    /// let zero = ascii::Char::Digit0;
    /// let seven = ascii::Char::Digit7;
    /// let eight = ascii::Char::Digit8;
    /// let percent = ascii::Char::PercentSign;
    /// let lf = ascii::Char::LineFeed;
    /// let esc = ascii::Char::Escape;
    ///
    /// assert!(!uppercase_a.is_octdigit());
    /// assert!(!a.is_octdigit());
    /// assert!(zero.is_octdigit());
    /// assert!(seven.is_octdigit());
    /// assert!(!eight.is_octdigit());
    /// assert!(!percent.is_octdigit());
    /// assert!(!lf.is_octdigit());
    /// assert!(!esc.is_octdigit());
    /// ```
    #[must_use]
    // 这受两个 unstable feature 阻塞。标记本方法为 stable 前,请确认二者都已稳定。
    #[unstable(feature = "ascii_char", issue = "110998")]
    // #[unstable(feature = "is_ascii_octdigit", issue = "101288")]
    #[inline]
    pub const fn is_octdigit(self) -> bool {
        self.to_u8().is_ascii_octdigit()
    }

    /// 检查本值是否是 ASCII 十六进制数字:
    ///
    /// - 0x30 '0' ..= 0x39 '9', or
    /// - 0x41 'A' ..= 0x46 'F', or
    /// - 0x61 'a' ..= 0x66 'f'.
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let uppercase_a = ascii::Char::CapitalA;
    /// let uppercase_g = ascii::Char::CapitalG;
    /// let a = ascii::Char::SmallA;
    /// let g = ascii::Char::SmallG;
    /// let zero = ascii::Char::Digit0;
    /// let percent = ascii::Char::PercentSign;
    /// let space = ascii::Char::Space;
    /// let lf = ascii::Char::LineFeed;
    /// let esc = ascii::Char::Escape;
    ///
    /// assert!(uppercase_a.is_hexdigit());
    /// assert!(!uppercase_g.is_hexdigit());
    /// assert!(a.is_hexdigit());
    /// assert!(!g.is_hexdigit());
    /// assert!(zero.is_hexdigit());
    /// assert!(!percent.is_hexdigit());
    /// assert!(!space.is_hexdigit());
    /// assert!(!lf.is_hexdigit());
    /// assert!(!esc.is_hexdigit());
    /// ```
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn is_hexdigit(self) -> bool {
        self.to_u8().is_ascii_hexdigit()
    }

    /// 检查本值是否是 ASCII 标点字符:
    ///
    /// - 0x21 ..= 0x2F `! " # $ % & ' ( ) * + , - . /`, or
    /// - 0x3A ..= 0x40 `: ; < = > ? @`, or
    /// - 0x5B ..= 0x60 `` [ \ ] ^ _ ` ``, or
    /// - 0x7B ..= 0x7E `{ | } ~`
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let uppercase_a = ascii::Char::CapitalA;
    /// let uppercase_g = ascii::Char::CapitalG;
    /// let a = ascii::Char::SmallA;
    /// let g = ascii::Char::SmallG;
    /// let zero = ascii::Char::Digit0;
    /// let percent = ascii::Char::PercentSign;
    /// let space = ascii::Char::Space;
    /// let lf = ascii::Char::LineFeed;
    /// let esc = ascii::Char::Escape;
    ///
    /// assert!(!uppercase_a.is_punctuation());
    /// assert!(!uppercase_g.is_punctuation());
    /// assert!(!a.is_punctuation());
    /// assert!(!g.is_punctuation());
    /// assert!(!zero.is_punctuation());
    /// assert!(percent.is_punctuation());
    /// assert!(!space.is_punctuation());
    /// assert!(!lf.is_punctuation());
    /// assert!(!esc.is_punctuation());
    /// ```
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn is_punctuation(self) -> bool {
        self.to_u8().is_ascii_punctuation()
    }

    /// 检查本值是否是 ASCII 图形字符:
    /// 0x21 '!' ..= 0x7E '~'.
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let uppercase_a = ascii::Char::CapitalA;
    /// let uppercase_g = ascii::Char::CapitalG;
    /// let a = ascii::Char::SmallA;
    /// let g = ascii::Char::SmallG;
    /// let zero = ascii::Char::Digit0;
    /// let percent = ascii::Char::PercentSign;
    /// let space = ascii::Char::Space;
    /// let lf = ascii::Char::LineFeed;
    /// let esc = ascii::Char::Escape;
    ///
    /// assert!(uppercase_a.is_graphic());
    /// assert!(uppercase_g.is_graphic());
    /// assert!(a.is_graphic());
    /// assert!(g.is_graphic());
    /// assert!(zero.is_graphic());
    /// assert!(percent.is_graphic());
    /// assert!(!space.is_graphic());
    /// assert!(!lf.is_graphic());
    /// assert!(!esc.is_graphic());
    /// ```
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn is_graphic(self) -> bool {
        self.to_u8().is_ascii_graphic()
    }

    /// 检查本值是否是 ASCII 空白字符:
    /// 0x20 SPACE, 0x09 HORIZONTAL TAB, 0x0A LINE FEED,
    /// 0x0C FORM FEED,或 0x0D CARRIAGE RETURN。
    ///
    /// Rust 使用 WhatWG Infra Standard 中对 [ASCII whitespace][infra-aw] 的定义。
    /// 现实中还有多种广泛使用的定义。例如 [POSIX locale][pct] 除上述字符外还包含
    /// 0x0B VERTICAL TAB;但同一规范下,[Bourne shell 的默认 "field splitting" 规则][bfs]
    /// 又*只*把 SPACE、HORIZONTAL TAB 和 LINE FEED 视为空白。
    ///
    /// 如果程序要处理既有文件格式,在使用本函数前应先确认该格式如何定义空白字符。
    ///
    /// [infra-aw]: https://infra.spec.whatwg.org/#ascii-whitespace
    /// [pct]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap07.html#tag_07_03_01
    /// [bfs]: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html#tag_18_06_05
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let uppercase_a = ascii::Char::CapitalA;
    /// let uppercase_g = ascii::Char::CapitalG;
    /// let a = ascii::Char::SmallA;
    /// let g = ascii::Char::SmallG;
    /// let zero = ascii::Char::Digit0;
    /// let percent = ascii::Char::PercentSign;
    /// let space = ascii::Char::Space;
    /// let lf = ascii::Char::LineFeed;
    /// let esc = ascii::Char::Escape;
    ///
    /// assert!(!uppercase_a.is_whitespace());
    /// assert!(!uppercase_g.is_whitespace());
    /// assert!(!a.is_whitespace());
    /// assert!(!g.is_whitespace());
    /// assert!(!zero.is_whitespace());
    /// assert!(!percent.is_whitespace());
    /// assert!(space.is_whitespace());
    /// assert!(lf.is_whitespace());
    /// assert!(!esc.is_whitespace());
    /// ```
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn is_whitespace(self) -> bool {
        self.to_u8().is_ascii_whitespace()
    }

    /// 检查本值是否是 ASCII 控制字符:
    /// 0x00 NUL ..= 0x1F UNIT SEPARATOR,或 0x7F DELETE。
    /// 注意:大多数空白字符也是控制字符,但 SPACE 不是。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let uppercase_a = ascii::Char::CapitalA;
    /// let uppercase_g = ascii::Char::CapitalG;
    /// let a = ascii::Char::SmallA;
    /// let g = ascii::Char::SmallG;
    /// let zero = ascii::Char::Digit0;
    /// let percent = ascii::Char::PercentSign;
    /// let space = ascii::Char::Space;
    /// let lf = ascii::Char::LineFeed;
    /// let esc = ascii::Char::Escape;
    ///
    /// assert!(!uppercase_a.is_control());
    /// assert!(!uppercase_g.is_control());
    /// assert!(!a.is_control());
    /// assert!(!g.is_control());
    /// assert!(!zero.is_control());
    /// assert!(!percent.is_control());
    /// assert!(!space.is_control());
    /// assert!(lf.is_control());
    /// assert!(esc.is_control());
    /// ```
    #[must_use]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn is_control(self) -> bool {
        self.to_u8().is_ascii_control()
    }

    /// 返回一个迭代器,逐字节产出该字符的转义表示。
    ///
    /// 行为与 [`ascii::escape_default`](crate::ascii::escape_default) 相同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ascii_char, ascii_char_variants)]
    /// use std::ascii;
    ///
    /// let zero = ascii::Char::Digit0;
    /// let tab = ascii::Char::CharacterTabulation;
    /// let cr = ascii::Char::CarriageReturn;
    /// let lf = ascii::Char::LineFeed;
    /// let apostrophe = ascii::Char::Apostrophe;
    /// let double_quote = ascii::Char::QuotationMark;
    /// let backslash = ascii::Char::ReverseSolidus;
    ///
    /// assert_eq!("0", zero.escape_ascii().to_string());
    /// assert_eq!("\\t", tab.escape_ascii().to_string());
    /// assert_eq!("\\r", cr.escape_ascii().to_string());
    /// assert_eq!("\\n", lf.escape_ascii().to_string());
    /// assert_eq!("\\'", apostrophe.escape_ascii().to_string());
    /// assert_eq!("\\\"", double_quote.escape_ascii().to_string());
    /// assert_eq!("\\\\", backslash.escape_ascii().to_string());
    /// ```
    #[must_use = "this returns the escaped character as an iterator, \
                  without modifying the original"]
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub fn escape_ascii(self) -> super::EscapeDefault {
        super::escape_default(self.to_u8())
    }
}

macro_rules! into_int_impl {
    ($($ty:ty)*) => {
        $(
            #[unstable(feature = "ascii_char", issue = "110998")]
            #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
            impl const From<AsciiChar> for $ty {
                #[inline]
                fn from(chr: AsciiChar) -> $ty {
                    chr as u8 as $ty
                }
            }
        )*
    }
}

into_int_impl!(u8 u16 u32 u64 u128 char);

impl [AsciiChar] {
    /// 把这个 ASCII 字符切片视为 UTF-8 `str`。
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn as_str(&self) -> &str {
        let ascii_ptr: *const Self = self;
        let str_ptr = ascii_ptr as *const str;
        // SAFETY: 每个 ASCII 码位在 UTF-8 中都编码为单个字节,且该字节值与 ASCII 值相同。
        unsafe { &*str_ptr }
    }

    /// 把这个 ASCII 字符切片视为 `u8` 字节切片。
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[inline]
    pub const fn as_bytes(&self) -> &[u8] {
        self.as_str().as_bytes()
    }
}

#[unstable(feature = "ascii_char", issue = "110998")]
impl fmt::Display for AsciiChar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <str as fmt::Display>::fmt(self.as_str(), f)
    }
}

#[unstable(feature = "ascii_char", issue = "110998")]
impl fmt::Debug for AsciiChar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use AsciiChar::{Apostrophe, Null, ReverseSolidus as Backslash};

        fn backslash(a: AsciiChar) -> ([AsciiChar; 6], usize) {
            ([Apostrophe, Backslash, a, Apostrophe, Null, Null], 4)
        }

        let (buf, len) = match self {
            AsciiChar::Null => backslash(AsciiChar::Digit0),
            AsciiChar::CharacterTabulation => backslash(AsciiChar::SmallT),
            AsciiChar::CarriageReturn => backslash(AsciiChar::SmallR),
            AsciiChar::LineFeed => backslash(AsciiChar::SmallN),
            AsciiChar::ReverseSolidus => backslash(AsciiChar::ReverseSolidus),
            AsciiChar::Apostrophe => backslash(AsciiChar::Apostrophe),
            _ if self.to_u8().is_ascii_control() => {
                const HEX_DIGITS: [AsciiChar; 16] = *b"0123456789abcdef".as_ascii().unwrap();

                let byte = self.to_u8();
                let hi = HEX_DIGITS[usize::from(byte >> 4)];
                let lo = HEX_DIGITS[usize::from(byte & 0xf)];
                ([Apostrophe, Backslash, AsciiChar::SmallX, hi, lo, Apostrophe], 6)
            }
            _ => ([Apostrophe, *self, Apostrophe, Null, Null, Null], 3),
        };

        f.write_str(buf[..len].as_str())
    }
}
