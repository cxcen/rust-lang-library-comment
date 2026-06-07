//! 字符串格式化与打印的核心工具。
//!
//! `core::fmt` 定义的是格式化协议本身:各类格式化 trait、`Formatter` 状态、
//! `Arguments` 的预编译表示,以及 `write!`/`format_args!` 依赖的无分配写入接口。
//! 它不负责分配字符串,也不直接访问 OS;panic 消息、日志框架以及 `std` 中的
//! `format!`/`println!` 最终都会经由这里的协议把值写入某个输出目标。

#![stable(feature = "rust1", since = "1.0.0")]

use crate::cell::{Cell, Ref, RefCell, RefMut, SyncUnsafeCell, UnsafeCell};
use crate::char::EscapeDebugExtArgs;
use crate::hint::assert_unchecked;
use crate::marker::{PhantomData, PointeeSized};
use crate::num::fmt as numfmt;
use crate::ops::Deref;
use crate::ptr::NonNull;
use crate::{iter, mem, result, str};

mod builders;
#[cfg(not(no_fp_fmt_parse))]
mod float;
#[cfg(no_fp_fmt_parse)]
mod nofloat;
mod num;
mod num_buffer;
mod rt;

#[stable(feature = "fmt_flags_align", since = "1.28.0")]
#[rustc_diagnostic_item = "Alignment"]
/// `Formatter::align` 可能返回的对齐方式。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Alignment {
    #[stable(feature = "fmt_flags_align", since = "1.28.0")]
    /// 表示内容应当左对齐。
    Left,
    #[stable(feature = "fmt_flags_align", since = "1.28.0")]
    /// 表示内容应当右对齐。
    Right,
    #[stable(feature = "fmt_flags_align", since = "1.28.0")]
    /// 表示内容应当居中对齐。
    Center,
}

#[unstable(feature = "int_format_into", issue = "138215")]
pub use num_buffer::{NumBuffer, NumBufferTrait};

#[stable(feature = "debug_builders", since = "1.2.0")]
pub use self::builders::{DebugList, DebugMap, DebugSet, DebugStruct, DebugTuple};
#[stable(feature = "fmt_from_fn", since = "1.93.0")]
pub use self::builders::{FromFn, from_fn};

/// 格式化方法返回的类型。
///
/// `fmt::Result` 只表达“向 `Formatter` 背后的输出目标写入是否继续成功”。
/// 它不是 `Display` 或 `Debug` 实现用来报告业务错误的通道。
///
/// # 示例
///
/// ```
/// use std::fmt;
///
/// #[derive(Debug)]
/// struct Triangle {
///     a: f32,
///     b: f32,
///     c: f32
/// }
///
/// impl fmt::Display for Triangle {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "({}, {}, {})", self.a, self.b, self.c)
///     }
/// }
///
/// let pythagorean_triple = Triangle { a: 3.0, b: 4.0, c: 5.0 };
///
/// assert_eq!(format!("{pythagorean_triple}"), "(3, 4, 5)");
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub type Result = result::Result<(), Error>;

/// 把消息格式化到某个流时可能返回的错误类型。
///
/// 这个类型除了“发生过某个错误”之外,不承载任何额外错误信息。原因是:
/// 虽然存在这个错误类型,字符串格式化本身仍被视为不可失败的操作。
/// `fmt()` 实现者只有在从传入的 [`Formatter`] 收到这个 `Error` 时才应返回它。
/// 代码主动创建新的 `fmt::Error` 的主要场景,是实现 `fmt::Write` 时底层流拒绝
/// 继续接收文本,从而需要中止整次格式化操作。
///
/// 如果需要传递额外信息,必须通过其他机制安排,例如把具体错误存入某个字段,
/// 待格式化操作被取消后再读取。[`std::io::Write::write_fmt()`] 在写入过程中
/// 传播 IO 错误时,采用的就是这种模式。
///
/// 不要把 `fmt::Error` 与可能同样在作用域中的 [`std::io::Error`] 或
/// [`std::error::Error`] 混淆。前者只是格式化协议里的中止信号,后两者才携带
/// 具体错误种类或错误链。
///
/// [`std::io::Error`]: ../../std/io/struct.Error.html
/// [`std::io::Write::write_fmt()`]: ../../std/io/trait.Write.html#method.write_fmt
/// [`std::error::Error`]: ../../std/error/trait.Error.html
///
/// # 示例
///
/// ```rust
/// use std::fmt::{self, write};
///
/// let mut output = String::new();
/// if let Err(fmt::Error) = write(&mut output, format_args!("Hello {}!", "world")) {
///     panic!("An error occurred");
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Copy, Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Error;

/// 向接受 Unicode 的缓冲区或流写入/格式化文本的 trait。
///
/// 这个 trait 只接受 UTF-8 编码的数据,并且不可 [flushable]。如果目标只需要接收
/// Unicode 文本且不需要 flush 语义,应实现这个 trait;否则应实现 [`std::io::Write`]。
///
/// `write!` 会通过 `write_fmt` 把 `format_args!` 生成的 [`Arguments`] 交给这里。
/// 这也是 panic 文本和日志文本能够在无分配或延迟分配路径上写入最终 sink 的基础。
///
/// [`std::io::Write`]: ../../std/io/trait.Write.html
/// [flushable]: ../../std/io/trait.Write.html#tymethod.flush
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "FmtWrite"]
pub trait Write {
    /// 向此 writer 写入一个字符串切片,并返回写入是否成功。
    ///
    /// 只有整个字符串切片都被成功写入时,此方法才算成功。它不会在部分写入后
    /// 报告成功;要么写完全部数据,要么在底层目标拒绝继续接收文本时返回错误。
    ///
    /// # 错误
    ///
    /// 出错时,此函数会返回 [`std::fmt::Error`][Error]。
    ///
    /// 这个错误的目的,是在底层目标遇到某种无法继续接收文本的问题时中止格式化操作;
    /// 它并不传达“具体发生了什么错误”。至少在实现格式化 trait 时,通常应继续向上传播
    /// 这个错误,而不是在本层处理它。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt::{Error, Write};
    ///
    /// fn writer<W: Write>(f: &mut W, s: &str) -> Result<(), Error> {
    ///     f.write_str(s)
    /// }
    ///
    /// let mut buf = String::new();
    /// writer(&mut buf, "hola")?;
    /// assert_eq!(&buf, "hola");
    /// # std::fmt::Result::Ok(())
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn write_str(&mut self, s: &str) -> Result;

    /// 向此 writer 写入一个 [`char`],并返回写入是否成功。
    ///
    /// 单个 [`char`] 可能会被编码为多个字节。只有整个 UTF-8 字节序列都被成功写入时,
    /// 此方法才算成功;它不会在部分写入后报告成功。
    ///
    /// # 错误
    ///
    /// 出错时,此函数会返回 [`Error`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt::{Error, Write};
    ///
    /// fn writer<W: Write>(f: &mut W, c: char) -> Result<(), Error> {
    ///     f.write_char(c)
    /// }
    ///
    /// let mut buf = String::new();
    /// writer(&mut buf, 'a')?;
    /// writer(&mut buf, 'b')?;
    /// assert_eq!(&buf, "ab");
    /// # std::fmt::Result::Ok(())
    /// ```
    #[stable(feature = "fmt_write_char", since = "1.1.0")]
    fn write_char(&mut self, c: char) -> Result {
        self.write_str(c.encode_utf8(&mut [0; char::MAX_LEN_UTF8]))
    }

    /// 供 [`write!`] 宏配合此 trait 的实现者使用的衔接方法。
    ///
    /// 通常不应手动调用此方法,而应通过 [`write!`] 宏调用。宏会先用
    /// [`format_args!`] 构造 [`Arguments`],再把它传给 `write_fmt`。
    ///
    /// # 错误
    ///
    /// 出错时,此函数会返回 [`Error`]。细节见 [write_str](Write::write_str)。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt::{Error, Write};
    ///
    /// fn writer<W: Write>(f: &mut W, s: &str) -> Result<(), Error> {
    ///     f.write_fmt(format_args!("{s}"))
    /// }
    ///
    /// let mut buf = String::new();
    /// writer(&mut buf, "world")?;
    /// assert_eq!(&buf, "world");
    /// # std::fmt::Result::Ok(())
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn write_fmt(&mut self, args: Arguments<'_>) -> Result {
        // 对 `Sized` 类型使用 specialization,避免经由 `&mut self` 多走一层间接调用。
        trait SpecWriteFmt {
            fn spec_write_fmt(self, args: Arguments<'_>) -> Result;
        }

        impl<W: Write + ?Sized> SpecWriteFmt for &mut W {
            #[inline]
            default fn spec_write_fmt(mut self, args: Arguments<'_>) -> Result {
                if let Some(s) = args.as_statically_known_str() {
                    self.write_str(s)
                } else {
                    write(&mut self, args)
                }
            }
        }

        impl<W: Write> SpecWriteFmt for &mut W {
            #[inline]
            fn spec_write_fmt(self, args: Arguments<'_>) -> Result {
                if let Some(s) = args.as_statically_known_str() {
                    self.write_str(s)
                } else {
                    write(self, args)
                }
            }
        }

        self.spec_write_fmt(args)
    }
}

#[stable(feature = "fmt_write_blanket_impl", since = "1.4.0")]
impl<W: Write + ?Sized> Write for &mut W {
    fn write_str(&mut self, s: &str) -> Result {
        (**self).write_str(s)
    }

    fn write_char(&mut self, c: char) -> Result {
        (**self).write_char(c)
    }

    fn write_fmt(&mut self, args: Arguments<'_>) -> Result {
        (**self).write_fmt(args)
    }
}

/// [`Formatter`] 或 [`FormattingOptions`] 当前请求的符号显示方式。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[unstable(feature = "formatting_options", issue = "118117")]
pub enum Sign {
    /// 表示 `+` flag。
    Plus,
    /// 表示 `-` flag。
    Minus,
}

/// 指定 [`Debug`] trait 是否应使用小写/大写十六进制,还是使用普通整数格式。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[unstable(feature = "formatting_options", issue = "118117")]
pub enum DebugAsHex {
    /// 让 `Debug` trait 使用小写十六进制整数,类似 [`x?` 格式](../../std/fmt/index.html#formatting-traits)。
    Lower,
    /// 让 `Debug` trait 使用大写十六进制整数,类似 [`X?` 格式](../../std/fmt/index.html#formatting-traits)。
    Upper,
}

/// 格式化选项。
///
/// `FormattingOptions` 可以看作尚未附加 [`Write`] 目标的 [`Formatter`] 状态。
/// 它主要用于构造 `Formatter` 实例,保存 width、precision、fill、alignment、
/// `#`、`0`、符号和 Debug 十六进制模式等格式化控制位。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[unstable(feature = "formatting_options", issue = "118117")]
pub struct FormattingOptions {
    /// flags 字段,按如下 bit field 编码:
    ///
    /// ```text
    ///   31  30  29  28  27  26  25  24  23  22  21  20                              0
    /// ┌───┬───────┬───┬───┬───┬───┬───┬───┬───┬───┬──────────────────────────────────┐
    /// │ 0 │ align │ p │ w │ X?│ x?│'0'│ # │ - │ + │               fill               │
    /// └───┴───────┴───┴───┴───┴───┴───┴───┴───┴───┴──────────────────────────────────┘
    ///   │     │     │   │  └─┬───────────────────┘ └─┬──────────────────────────────┘
    ///   │     │     │   │    │                       └─ fill 字符(21 位 char)。
    ///   │     │     │   │    └─ Debug 大/小写十六进制、补零、alternate、正负号 flags。
    ///   │     │     │   └─ 是否设置了 width。(具体值单独存储。)
    ///   │     │     └─ 是否设置了 precision。(具体值单独存储。)
    ///   │     ├─ 0: 左对齐。(<)
    ///   │     ├─ 1: 右对齐。(>)
    ///   │     ├─ 2: 居中对齐。(^)
    ///   │     └─ 3: 未设置 alignment。(默认)
    ///   └─ 始终为 0。
    /// ```
    // 注意:这里可以使用范围为 0x0000_0000..=0x7dd0ffff 的 pattern type,
    // 但目前不清楚那是否有实际价值。
    flags: u32,
    /// 若上面的 width flag(bit 27) 已设置,则保存 width;否则始终为 0。
    width: u16,
    /// 若上面的 precision flag(bit 28) 已设置,则保存 precision;否则始终为 0。
    precision: u16,
}

// 这里必须与 compiler/rustc_ast_lowering/src/format.rs 保持一致。
mod flags {
    pub(super) const SIGN_PLUS_FLAG: u32 = 1 << 21;
    pub(super) const SIGN_MINUS_FLAG: u32 = 1 << 22;
    pub(super) const ALTERNATE_FLAG: u32 = 1 << 23;
    pub(super) const SIGN_AWARE_ZERO_PAD_FLAG: u32 = 1 << 24;
    pub(super) const DEBUG_LOWER_HEX_FLAG: u32 = 1 << 25;
    pub(super) const DEBUG_UPPER_HEX_FLAG: u32 = 1 << 26;
    pub(super) const WIDTH_FLAG: u32 = 1 << 27;
    pub(super) const PRECISION_FLAG: u32 = 1 << 28;
    pub(super) const ALIGN_BITS: u32 = 0b11 << 29;
    pub(super) const ALIGN_LEFT: u32 = 0 << 29;
    pub(super) const ALIGN_RIGHT: u32 = 1 << 29;
    pub(super) const ALIGN_CENTER: u32 = 2 << 29;
    pub(super) const ALIGN_UNKNOWN: u32 = 3 << 29;
}

impl FormattingOptions {
    /// 构造一组等价于 `{}` 格式说明符的默认格式化选项。
    ///
    /// - 没有 flags,
    /// - 使用空格填充,
    /// - 没有 alignment,
    /// - 没有 width,
    /// - 没有 precision,
    /// - 没有 [`DebugAsHex`] 输出模式。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn new() -> Self {
        Self { flags: ' ' as u32 | flags::ALIGN_UNKNOWN, width: 0, precision: 0 }
    }

    /// 设置或移除符号 flag(`+` 或 `-`)。
    ///
    /// - `+`: 面向数值类型,表示始终打印符号。默认情况下,只有有符号值为负时
    ///   才会打印负号,正数和无符号值的符号会被省略。设置此 flag 后,
    ///   应始终打印正确符号(+ 或 -)。
    /// - `-`: 当前未使用。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn sign(&mut self, sign: Option<Sign>) -> &mut Self {
        let sign = match sign {
            None => 0,
            Some(Sign::Plus) => flags::SIGN_PLUS_FLAG,
            Some(Sign::Minus) => flags::SIGN_MINUS_FLAG,
        };
        self.flags = self.flags & !(flags::SIGN_PLUS_FLAG | flags::SIGN_MINUS_FLAG) | sign;
        self
    }
    /// 设置或取消 `0` flag。
    ///
    /// 对整数格式而言,这表示为了达到 width 而进行的填充应使用 `0` 字符,
    /// 并且应感知符号位置,即符号和前缀通常位于补零之前。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn sign_aware_zero_pad(&mut self, sign_aware_zero_pad: bool) -> &mut Self {
        if sign_aware_zero_pad {
            self.flags |= flags::SIGN_AWARE_ZERO_PAD_FLAG;
        } else {
            self.flags &= !flags::SIGN_AWARE_ZERO_PAD_FLAG;
        }
        self
    }
    /// 设置或取消 `#` flag。
    ///
    /// 这个 flag 表示应使用“alternate”形式打印。常见 alternate 形式包括:
    /// - [`Debug`]: pretty-print [`Debug`] 格式化结果,添加换行和缩进。
    /// - [`LowerHex`] 与 [`UpperHex`]: 在参数前添加 `0x`。
    /// - [`Octal`]: 在参数前添加 `0o`。
    /// - [`Binary`]: 在参数前添加 `0b`。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn alternate(&mut self, alternate: bool) -> &mut Self {
        if alternate {
            self.flags |= flags::ALTERNATE_FLAG;
        } else {
            self.flags &= !flags::ALTERNATE_FLAG;
        }
        self
    }
    /// 设置填充字符。
    ///
    /// 可选填充字符通常与 alignment 和 width 一起提供。若被格式化值的文本长度
    /// 小于 width,`Formatter` 会在其周围写入额外填充字符以补足宽度。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn fill(&mut self, fill: char) -> &mut Self {
        self.flags = self.flags & (u32::MAX << 21) | fill as u32;
        self
    }
    /// 设置或移除 alignment。
    ///
    /// 当被格式化值短于 `Formatter` 的 width 时,alignment 指定该值应如何摆放。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn align(&mut self, align: Option<Alignment>) -> &mut Self {
        let align: u32 = match align {
            Some(Alignment::Left) => flags::ALIGN_LEFT,
            Some(Alignment::Right) => flags::ALIGN_RIGHT,
            Some(Alignment::Center) => flags::ALIGN_CENTER,
            None => flags::ALIGN_UNKNOWN,
        };
        self.flags = self.flags & !flags::ALIGN_BITS | align;
        self
    }
    /// 设置或移除 width。
    ///
    /// 这是格式化结果应占据的“最小宽度”。如果值的字符串表示没有填满这么多字符,
    /// 就会使用 [`FormattingOptions::fill`] 和 [`FormattingOptions::align`]
    /// 指定的规则补足所需空间。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn width(&mut self, width: Option<u16>) -> &mut Self {
        if let Some(width) = width {
            self.flags |= flags::WIDTH_FLAG;
            self.width = width;
        } else {
            self.flags &= !flags::WIDTH_FLAG;
            self.width = 0;
        }
        self
    }
    /// 设置或移除 precision。
    ///
    /// - 对非数值类型,precision 可视为“最大宽度”。若结果字符串长于此宽度,
    ///   会先截断到这么多字符,再结合已设置的 fill、alignment 和 width 输出。
    /// - 对整数类型,precision 会被忽略。
    /// - 对浮点类型,precision 表示小数点后应打印多少位数字。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn precision(&mut self, precision: Option<u16>) -> &mut Self {
        if let Some(precision) = precision {
            self.flags |= flags::PRECISION_FLAG;
            self.precision = precision;
        } else {
            self.flags &= !flags::PRECISION_FLAG;
            self.precision = 0;
        }
        self
    }
    /// 指定 [`Debug`] trait 应使用小写/大写十六进制,还是使用普通整数。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn debug_as_hex(&mut self, debug_as_hex: Option<DebugAsHex>) -> &mut Self {
        let debug_as_hex = match debug_as_hex {
            None => 0,
            Some(DebugAsHex::Lower) => flags::DEBUG_LOWER_HEX_FLAG,
            Some(DebugAsHex::Upper) => flags::DEBUG_UPPER_HEX_FLAG,
        };
        self.flags = self.flags & !(flags::DEBUG_LOWER_HEX_FLAG | flags::DEBUG_UPPER_HEX_FLAG)
            | debug_as_hex;
        self
    }

    /// 返回当前符号 flag(`+` 或 `-`)。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn get_sign(&self) -> Option<Sign> {
        if self.flags & flags::SIGN_PLUS_FLAG != 0 {
            Some(Sign::Plus)
        } else if self.flags & flags::SIGN_MINUS_FLAG != 0 {
            Some(Sign::Minus)
        } else {
            None
        }
    }
    /// 返回当前 `0` flag。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn get_sign_aware_zero_pad(&self) -> bool {
        self.flags & flags::SIGN_AWARE_ZERO_PAD_FLAG != 0
    }
    /// 返回当前 `#` flag。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn get_alternate(&self) -> bool {
        self.flags & flags::ALTERNATE_FLAG != 0
    }
    /// 返回当前填充字符。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn get_fill(&self) -> char {
        // SAFETY: 我们只会把有效 `char` 放入 flags 字段的低 21 位。
        unsafe { char::from_u32_unchecked(self.flags & 0x1FFFFF) }
    }
    /// 返回当前 alignment。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn get_align(&self) -> Option<Alignment> {
        match self.flags & flags::ALIGN_BITS {
            flags::ALIGN_LEFT => Some(Alignment::Left),
            flags::ALIGN_RIGHT => Some(Alignment::Right),
            flags::ALIGN_CENTER => Some(Alignment::Center),
            _ => None,
        }
    }
    /// 返回当前 width。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn get_width(&self) -> Option<u16> {
        if self.flags & flags::WIDTH_FLAG != 0 { Some(self.width) } else { None }
    }
    /// 返回当前 precision。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn get_precision(&self) -> Option<u16> {
        if self.flags & flags::PRECISION_FLAG != 0 { Some(self.precision) } else { None }
    }
    /// 返回当前 Debug 十六进制输出模式。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn get_debug_as_hex(&self) -> Option<DebugAsHex> {
        if self.flags & flags::DEBUG_LOWER_HEX_FLAG != 0 {
            Some(DebugAsHex::Lower)
        } else if self.flags & flags::DEBUG_UPPER_HEX_FLAG != 0 {
            Some(DebugAsHex::Upper)
        } else {
            None
        }
    }

    /// 创建一个把输出写入给定 [`Write`] trait 对象的 [`Formatter`]。
    ///
    /// 也可以改用 [`Formatter::new()`]。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn create_formatter<'a>(self, write: &'a mut (dyn Write + 'a)) -> Formatter<'a> {
        Formatter { options: self, buf: write }
    }
}

#[unstable(feature = "formatting_options", issue = "118117")]
impl Default for FormattingOptions {
    /// 等同于 [`FormattingOptions::new()`]。
    fn default() -> Self {
        // `#[derive(Default)]` 实现会把 `fill` 设成 `\0` 而不是空格。
        Self::new()
    }
}

/// 格式化配置和输出目标。
///
/// `Formatter` 表示一次格式化调用的状态:width、precision、fill、alignment、
/// flags 以及底层 [`Write`] sink。用户通常不会直接构造 `Formatter`;所有格式化
/// trait 的 `fmt` 方法,例如 [`Debug`] 和 [`Display`],都会接收一个
/// `&mut Formatter`。
///
/// `Display` 实现通常生成面向用户的、稳定且可读的文本;`Debug` 实现则面向开发者
/// 和诊断场景,会尊重 `#?` 等调试 flag。与 `Formatter` 交互时,可以调用下方方法
/// 查询这些状态,或派生带不同 [`FormattingOptions`] 的临时 `Formatter`。
#[allow(missing_debug_implementations)]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "Formatter"]
pub struct Formatter<'a> {
    options: FormattingOptions,

    buf: &'a mut (dyn Write + 'a),
}

impl<'a> Formatter<'a> {
    /// 使用给定 [`FormattingOptions`] 创建新的 `Formatter`。
    ///
    /// 如果 `write` 本身是对另一个 formatter 的引用,建议改用
    /// [`Formatter::with_options`],因为它可以直接借用底层 `write`,
    /// 从而绕过一层间接调用。
    ///
    /// 也可以改用 [`FormattingOptions::create_formatter()`]。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn new(write: &'a mut (dyn Write + 'a), options: FormattingOptions) -> Self {
        Formatter { options, buf: write }
    }

    /// 基于当前 formatter 和给定 [`FormattingOptions`] 创建新的 formatter。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn with_options<'b>(&'b mut self, options: FormattingOptions) -> Formatter<'b> {
        Formatter { options, buf: self.buf }
    }
}

/// 表示格式字符串及其实参的安全预编译版本。
///
/// 这个结构不能在运行时任意生成,因为运行时无法重新证明模板编码、参数类型和
/// 生命周期都匹配;因此它没有公开构造器,字段也保持私有以防被修改。
///
/// [`format_args!`] 宏会安全地创建此结构的实例。宏在编译期验证格式字符串,
/// 从而让 [`write()`] 和 [`format()`] 能安全消费该结构。
///
/// [`format_args!`] 返回的 `Arguments<'a>` 可以在 `Debug` 和 `Display` 上下文中使用,
/// 如下例所示。示例也展示了 `Arguments` 自身的 `Debug` 和 `Display` 输出相同:
/// 它们都会输出 `format_args!` 插值后的格式字符串。
///
/// 生命周期 `'a` 来自被格式化实参的借用。通常应立即把 `Arguments` 传给
/// `write!`、日志或 panic 设施;除纯静态字符串表示外,不要把它保存得比实参更久。
///
/// ```rust
/// let debug = format!("{:?}", format_args!("{} foo {:?}", 1, 2));
/// let display = format!("{}", format_args!("{} foo {:?}", 1, 2));
/// assert_eq!("1 foo 2", display);
/// assert_eq!(display, debug);
/// ```
///
/// [`format()`]: ../../std/fmt/fn.format.html
//
// 内部表示:
//
// fmt::Arguments 有两种表示方式:
//
// 1) 字符串字面量表示(例如 format_args!("hello"))
//             ┌────────────────────────────────┐
//   template: │           *const u8            │ ─▷ "hello"
//             ├──────────────────────────────┬─┤
//   args:     │             len              │1│ (最低位为 1;字段包含 `len << 1 | 1`)
//             └──────────────────────────────┴─┘
//   在这种表示中没有占位符,`fmt::Arguments::as_str()` 会返回 Some。
//   指针指向一个静态 `str` 的起始位置。长度由 `args as usize >> 1` 给出。
//   (`&str` 的长度最大为 isize::MAX,因此总能放入少一位的 usize。)
//
//   `fmt::Arguments::from_str()` 会从 `&'static str` 构造这种表示。
//
// 2) 占位符表示(例如 format_args!("hello {name}\n"))
//             ┌────────────────────────────────┐
//   template: │           *const u8            │ ─▷ b"\x06hello \xC0\x01\n\x00"
//             ├────────────────────────────────┤
//   args:     │     &'a [Argument<'a>; _]     0│ (因 Argument 类型对齐要求,低位为 0)
//             └────────────────────────────────┘
//   在这种表示中,template 是一段同时编码字面量字符串片段和占位符
//   (包括其 options/flags)的字节序列。
//
//   `args` 指针指向一个 `fmt::Argument<'a>` 数组,其长度必须足以匹配 template 中的占位符。
//
//   `fmt::Arguments::new()` 从 template 字节切片和 arguments 切片构造这种表示。
//   该函数是 unsafe 的,因为它假定 template 已经合法,且 args 切片中的元素与 template 匹配。
//
//   template 字节序列由以下类型的片段拼接而成:
//
//   - 字面量字符串片段:
//         必须原样格式化的片段(例如 "hello {name}\n" 中的 "hello " 和 "\n")
//         会按字面值出现在 template 字节序列中,前面带有长度。
//
//         对最多 127 字节的片段,用一个包含长度的单字节表示,后面直接跟字符串字节:
//         ┌───┬────────────────────────────┐
//         │len│    `len` bytes (utf-8)     │ (e.g. b"\x06hello ")
//         └───┴────────────────────────────┘
//
//         对更大的、最多 u16::MAX 字节的片段,用 0x80 后跟 16 位小端长度表示,
//         再跟字符串字节:
//         ┌────┬─────────┬───────────────────────────┐
//         │0x80│   len   │   `len` bytes (utf-8)     │ (e.g. b"\x80\x00\x01hello … ")
//         └────┴─────────┴───────────────────────────┘
//
//         更长片段会在 UTF-8 边界上拆分为多个最大 u16::MAX 字节的片段。
//
//   - 占位符:
//         占位符(例如 "hello {name}" 中的 `{name}`)表示为最高两位已设置的一个字节,
//         后面根据首字节中的 flags 跟随零个或多个字段:
//         ┌──────────┬┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┬┄┄┄┄┄┄┄┄┄┄┄┬┄┄┄┄┄┄┄┄┄┄┄┬┄┄┄┄┄┄┄┄┄┄┄┐
//         │0b11______│       flags       ┊   width   ┊ precision ┊ arg_index ┊ (e.g. b"\xC2\x05\0")
//         └────││││││┴┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┴┄┄┄┄┄┄┄┄┄┄┄┴┄┄┄┄┄┄┄┄┄┄┄┴┄┄┄┄┄┄┄┄┄┄┄┘
//              ││││││        32 bit          16 bit      16 bit      16 bit
//              │││││└─ 存在 flags
//              ││││└─ 存在 width
//              │││└─ 存在 precision
//              ││└─ 存在 arg_index
//              │└─ width 为间接索引
//              └─ precision 为间接索引
//
//         除首字节外,其他字段都是可选的;只有首字节中对应 flag 被设置时才会出现。
//
//         因此,没有任何 options 的完全默认占位符只是一个单字节:
//         ┌──────────┐
//         │0b11000000│ (b"\xC0")
//         └──────────┘
//
//         各字段按小端格式存储。
//
//         `flags` 字段对应 `FormattingOptions` 的 `flags` 字段。细节见
//         `FormattingOptions::flags` 的文档注释。
//
//         `width` 和 `precision` 字段分别对应 `FormattingOptions` 中的同名字段。
//         但若它们的“indirect” flag 被设置,字段中保存的是动态 width 或 precision
//         在 `args` 数组中的索引,而不是直接值。
//
//         `arg_index` 字段是待格式化参数在 `args` 数组中的索引。
//
//         若省略,则使用默认 `FormattingOptions::new()` 的 flags、width 和 precision。
//
//         若省略 `arg_index`,则使用 `args` 数组中的下一个参数(从 0 开始)。
//
//   - 结束:
//         单个零字节标记 template 结束:
//         ┌───┐
//         │ 0 │ ("\0")
//         └───┘
//
//         (注意,零字节也可能自然出现在上面的字符串片段或 flags、width、precision、
//         arg_index 字段中。也就是说,template 字节序列以 0 字节结束,
//         但不是由第一个 0 字节终止。)
//
#[lang = "format_arguments"]
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Copy, Clone)]
pub struct Arguments<'a> {
    template: NonNull<u8>,
    args: NonNull<rt::Argument<'a>>,
}

/// 供 format_args!() 宏创建 fmt::Arguments 对象时使用。
#[doc(hidden)]
#[rustc_diagnostic_item = "FmtArgumentsNew"]
#[unstable(feature = "fmt_internals", issue = "none")]
impl<'a> Arguments<'a> {
    // SAFETY: 调用方必须保证传入的 template 与 args 按上文所述编码出合法的
    // fmt::Arguments。
    #[inline]
    pub unsafe fn new<const N: usize, const M: usize>(
        template: &'a [u8; N],
        args: &'a [rt::Argument<'a>; M],
    ) -> Arguments<'a> {
        // SAFETY: 这是调用方的责任。
        unsafe { Arguments { template: mem::transmute(template), args: mem::transmute(args) } }
    }

    // 与 `from_str` 相同,但不是 const。
    // 当 format_args!() 展开中内联了参数时使用,例如 format_args!("{}", 123),
    // 这种形式不允许出现在 const 中。
    #[inline]
    pub fn from_str_nonconst(s: &'static str) -> Arguments<'a> {
        Arguments::from_str(s)
    }
}

#[doc(hidden)]
#[unstable(feature = "fmt_internals", issue = "none")]
impl<'a> Arguments<'a> {
    /// 估算格式化后文本的长度。
    ///
    /// 该值用于 `format!` 设置初始 `String` 容量。注意:这既不是下界也不是上界,
    /// 只是减少常见重新分配次数的启发式估算。
    #[inline]
    pub fn estimated_capacity(&self) -> usize {
        if let Some(s) = self.as_str() {
            return s.len();
        }
        // 遍历 template,统计字面量片段的长度。
        let mut length = 0usize;
        let mut starts_with_placeholder = false;
        let mut template = self.template;
        loop {
            // SAFETY: 可以假定 template 合法。
            unsafe {
                let n = template.read();
                template = template.add(1);
                if n == 0 {
                    // template 结束。
                    break;
                } else if n < 128 {
                    // 短字面量字符串片段。
                    length += n as usize;
                    template = template.add(n as usize);
                } else if n == 128 {
                    // 长字面量字符串片段。
                    let len = usize::from(u16::from_le_bytes(template.cast_array().read()));
                    length += len;
                    template = template.add(2 + len);
                } else {
                    assert_unchecked(n >= 0xC0);
                    // 占位符片段。
                    if length == 0 {
                        starts_with_placeholder = true;
                    }
                    // 跳过占位符剩余部分:
                    let skip = (n & 1 != 0) as usize * 4 // flags (32 bit)
                        + (n & 2 != 0) as usize * 2  // width     (16 bit)
                        + (n & 4 != 0) as usize * 2  // precision (16 bit)
                        + (n & 8 != 0) as usize * 2; // arg_index (16 bit)
                    template = template.add(skip as usize);
                }
            }
        }

        if starts_with_placeholder && length < 16 {
            // 如果格式字符串以占位符开头,且字面量片段长度并不显著,
            // 就不预分配任何容量。
            0
        } else {
            // 存在一些占位符时,后续 push 可能导致 String 重新分配。
            // 为避免这种情况,这里预先把容量“翻倍”。
            length.wrapping_mul(2)
        }
    }
}

impl<'a> Arguments<'a> {
    /// 为单个静态字符串创建 `fmt::Arguments` 对象。
    ///
    /// 格式化这个 `fmt::Arguments` 只会原样产生该字符串。
    #[inline]
    #[unstable(feature = "fmt_arguments_from_str", issue = "148905")]
    pub const fn from_str(s: &'static str) -> Arguments<'a> {
        // SAFETY: 这是 fmt::Arguments 的“static str”表示;见上面的编码说明。
        unsafe {
            Arguments {
                template: mem::transmute(s.as_ptr()),
                args: mem::transmute(s.len() << 1 | 1),
            }
        }
    }

    /// 若该值没有需要在运行时格式化的参数,则取得格式化后的字符串。
    ///
    /// 某些情况下可用它避免分配。
    ///
/// # 保证
    ///
    /// 对 `format_args!("just a literal")`,此函数保证返回
    /// `Some("just a literal")`。
    ///
    /// 对大多数带占位符的情况,此函数会返回 `None`。
    ///
    /// 不过编译器可能执行优化,使得即使格式字符串包含占位符,此函数也返回
    /// `Some(_)`。例如 `format_args!("Hello, {}!", "world")` 可能被优化为
    /// `format_args!("Hello, world!")`,从而让 `as_str()` 返回
    /// `Some("Hello, world!")`。
    ///
    /// 除无占位符的简单情况外,其他行为都不保证稳定,也不应被除优化以外的逻辑依赖。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::fmt::Arguments;
    ///
    /// fn write_str(_: &str) { /* ... */ }
    ///
    /// fn write_fmt(args: &Arguments<'_>) {
    ///     if let Some(s) = args.as_str() {
    ///         write_str(s)
    ///     } else {
    ///         write_str(&args.to_string());
    ///     }
    /// }
    /// ```
    ///
    /// ```rust
    /// assert_eq!(format_args!("hello").as_str(), Some("hello"));
    /// assert_eq!(format_args!("").as_str(), Some(""));
    /// assert_eq!(format_args!("{:?}", std::env::current_dir()).as_str(), None);
    /// ```
    #[stable(feature = "fmt_as_str", since = "1.52.0")]
    #[rustc_const_stable(feature = "const_arguments_as_str", since = "1.84.0")]
    #[must_use]
    #[inline]
    pub const fn as_str(&self) -> Option<&'static str> {
        // SAFETY: 在 const eval 期间,`self.args` 必须来自 usize 而不是指针,
        // 因为这是在 const 中创建 fmt::Arguments 的唯一方式。
        // (也就是说,只有 fmt::Arguments::from_str 是 const,fmt::Arguments::new 不是。)
        //
        // 在 const eval 之外,把指针 transmute 为 usize 是可以的。
        let bits: usize = unsafe { mem::transmute(self.args) };
        if bits & 1 == 1 {
            // SAFETY: 这个 fmt::Arguments 保存的是 &'static str。见上面的编码文档。
            Some(unsafe {
                str::from_utf8_unchecked(crate::slice::from_raw_parts(
                    self.template.as_ptr(),
                    bits >> 1,
                ))
            })
        } else {
            None
        }
    }

    /// 与 [`Arguments::as_str`] 相同,但只有在编译期能确定时才会返回 `Some(s)`。
    #[unstable(feature = "fmt_internals", reason = "internal to standard library", issue = "none")]
    #[must_use]
    #[inline]
    #[doc(hidden)]
    pub fn as_statically_known_str(&self) -> Option<&'static str> {
        let s = self.as_str();
        if core::intrinsics::is_val_statically_known(s.is_some()) { s } else { None }
    }
}

// 手写这些实现可以得到更好的错误消息。
#[stable(feature = "rust1", since = "1.0.0")]
impl !Send for Arguments<'_> {}
#[stable(feature = "rust1", since = "1.0.0")]
impl !Sync for Arguments<'_> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl Debug for Arguments<'_> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
        Display::fmt(self, fmt)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Display for Arguments<'_> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
        write(fmt.buf, *self)
    }
}

/// `?` 格式化。
///
/// `Debug` 应在面向程序员的调试上下文中格式化输出。
///
/// 一般来说,应优先直接 `derive` 一个 `Debug` 实现。
///
/// 与 alternate 格式说明符 `#?` 一起使用时,输出会被 pretty-print。
///
/// 关于 formatter 的更多信息,见[模块级文档][module]。
///
/// [module]: ../../std/fmt/index.html
///
/// 如果所有字段都实现了 `Debug`,这个 trait 可以配合 `#[derive]` 使用。对结构体
/// derive 时,输出会依次包含 `struct` 名、`{`、逗号分隔的字段名及其 `Debug` 值、
/// 然后是 `}`。对 `enum` 而言,输出会包含变体名;若该变体带字段,则还会包含 `(`
/// 字段的 `Debug` 值以及 `)`。
///
/// # 稳定性
///
/// derive 得到的 `Debug` 格式不保证稳定,未来 Rust 版本可能改变它。同样,
/// 标准库(`std`、`core`、`alloc` 等)提供类型的 `Debug` 实现也不保证稳定,
/// 未来版本可能改变。
///
/// # 示例
///
/// 派生一个实现:
///
/// ```
/// #[derive(Debug)]
/// struct Point {
///     x: i32,
///     y: i32,
/// }
///
/// let origin = Point { x: 0, y: 0 };
///
/// assert_eq!(
///     format!("The origin is: {origin:?}"),
///     "The origin is: Point { x: 0, y: 0 }",
/// );
/// ```
///
/// 手写一个实现:
///
/// ```
/// use std::fmt;
///
/// struct Point {
///     x: i32,
///     y: i32,
/// }
///
/// impl fmt::Debug for Point {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         f.debug_struct("Point")
///          .field("x", &self.x)
///          .field("y", &self.y)
///          .finish()
///     }
/// }
///
/// let origin = Point { x: 0, y: 0 };
///
/// assert_eq!(
///     format!("The origin is: {origin:?}"),
///     "The origin is: Point { x: 0, y: 0 }",
/// );
/// ```
///
/// [`Formatter`] 结构体上有若干辅助方法可帮助手写实现,例如 [`debug_struct`]。
///
/// [`debug_struct`]: Formatter::debug_struct
///
/// 不想使用 `Formatter` 提供的标准 debug 表示套件(`debug_struct`、`debug_tuple`、
/// `debug_list`、`debug_set`、`debug_map`)的类型,也可以手动向 `Formatter`
/// 写入任意自定义表示。
///
/// ```
/// # use std::fmt;
/// # struct Point {
/// #     x: i32,
/// #     y: i32,
/// # }
/// #
/// impl fmt::Debug for Point {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "Point [{} {}]", self.x, self.y)
///     }
/// }
/// ```
///
/// 通过 `derive` 或 [`Formatter`] 上 debug builder API 写出的 `Debug` 实现,
/// 都支持使用 alternate flag `{:#?}` 进行美化打印(pretty-print)。
///
/// 使用 `#?` 进行 pretty-print:
///
/// ```
/// #[derive(Debug)]
/// struct Point {
///     x: i32,
///     y: i32,
/// }
///
/// let origin = Point { x: 0, y: 0 };
///
/// let expected = "The origin is: Point {
///     x: 0,
///     y: 0,
/// }";
/// assert_eq!(format!("The origin is: {origin:#?}"), expected);
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_on_unimplemented(
    on(
        crate_local,
        note = "add `#[derive(Debug)]` to `{Self}` or manually `impl {This} for {Self}`"
    ),
    on(
        from_desugaring = "FormatLiteral",
        label = "`{Self}` cannot be formatted using `{{:?}}` because it doesn't implement `{This}`"
    ),
    message = "`{Self}` doesn't implement `{This}`"
)]
#[doc(alias = "{:?}")]
#[rustc_diagnostic_item = "Debug"]
#[rustc_trivial_field_reads]
pub trait Debug: PointeeSized {
    #[doc = include_str!("fmt_trait_method_doc.md")]
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Position {
    ///     longitude: f32,
    ///     latitude: f32,
    /// }
    ///
    /// impl fmt::Debug for Position {
    ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         f.debug_tuple("")
    ///          .field(&self.longitude)
    ///          .field(&self.latitude)
    ///          .finish()
    ///     }
    /// }
    ///
    /// let position = Position { longitude: 1.987, latitude: 2.983 };
    /// assert_eq!(format!("{position:?}"), "(1.987, 2.983)");
    ///
    /// assert_eq!(format!("{position:#?}"), "(
    ///     1.987,
    ///     2.983,
    /// )");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}

// 单独模块用于从 prelude 重新导出宏 `Debug`,但不重新导出 trait `Debug`。
pub(crate) mod macros {
    /// 生成 `Debug` trait 实现的 derive 宏。
    #[rustc_builtin_macro]
    #[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
    #[allow_internal_unstable(core_intrinsics, fmt_helpers_for_derive)]
    pub macro Debug($item:item) {
        /* compiler built-in */
    }
}
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[doc(inline)]
pub use macros::Debug;

/// 空格式 `{}` 对应的格式化 trait。
///
/// 为某个类型实现此 trait 会自动为该类型实现 [`ToString`][tostring] trait,
/// 从而允许使用 [`.to_string()`][tostring_function] 方法。应优先为类型实现
/// `Display` trait,而不是直接实现 [`ToString`][tostring]。
///
/// `Display` 与 [`Debug`] 类似,但 `Display` 面向用户可见输出,因此不能 derive。
/// 换言之,`Debug` 更强调诊断信息完整性和开发者可读性,而 `Display` 应表达该类型
/// 最自然、最适合展示给人的文本形式。
///
/// 关于 formatter 的更多信息,见[模块级文档][module]。
///
/// [module]: ../../std/fmt/index.html
/// [tostring]: ../../std/string/trait.ToString.html
/// [tostring_function]: ../../std/string/trait.ToString.html#tymethod.to_string
///
/// # 完整性与可解析性
///
/// 某个类型的 `Display` 不一定是该类型的无损或完整表示。类型可以按照自己的定义,
/// 省略不适合用户可见输出的内部状态、精度或其他信息。因此 `Display` 输出未必能被解析;
/// 即使能解析,解析结果也未必与原始值完全相同。
///
/// 但如果某个类型的 `Display` 实现是无损的,且其输出不仅供人阅读,也有意便于机器解析,
/// 那么该类型通常应考虑在 `FromStr` 中接受同一格式,并在文档中说明这一点。若同时存在
/// `Display` 和 `FromStr` 实现,但 `Display` 的结果无法被 `FromStr` 解析,可能会让用户意外。
///
/// # 国际化
///
/// 因为一个类型只能有一个 `Display` 实现,通常只有在值存在单一、最“显然”的文本格式时
/// 才适合实现 `Display`。这可能意味着按“invariant”文化和“undefined” locale 格式化,
/// 也可能意味着该类型的展示文本就是为某个特定文化/locale 设计的,例如开发者日志。
///
/// 如果并非所有值都有合理的规范文本格式,或者你想支持标准 [formatting traits]
/// 未覆盖的替代格式,最灵活的做法是提供 display adapter:例如
/// [`str::escape_default`] 或 [`Path::display`] 这类方法,创建一个实现 `Display`
/// 的包装器来输出特定展示格式。
///
/// [formatting traits]: ../../std/fmt/index.html#formatting-traits
/// [`Path::display`]: ../../std/path/struct.Path.html#method.display
///
/// # 示例
///
/// 为某个类型实现 `Display`:
///
/// ```
/// use std::fmt;
///
/// struct Point {
///     x: i32,
///     y: i32,
/// }
///
/// impl fmt::Display for Point {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "({}, {})", self.x, self.y)
///     }
/// }
///
/// let origin = Point { x: 0, y: 0 };
///
/// assert_eq!(format!("The origin is: {origin}"), "The origin is: (0, 0)");
/// ```
#[rustc_on_unimplemented(
    on(
        any(Self = "std::path::Path", Self = "std::path::PathBuf"),
        label = "`{Self}` cannot be formatted with the default formatter; call `.display()` on it",
        note = "call `.display()` or `.to_string_lossy()` to safely print paths, \
                as they may contain non-Unicode data",
    ),
    on(
        from_desugaring = "FormatLiteral",
        note = "in format strings you may be able to use `{{:?}}` (or {{:#?}} for pretty-print) instead",
        label = "`{Self}` cannot be formatted with the default formatter",
    ),
    message = "`{Self}` doesn't implement `{This}`"
)]
#[doc(alias = "{}")]
#[rustc_diagnostic_item = "Display"]
#[stable(feature = "rust1", since = "1.0.0")]
pub trait Display: PointeeSized {
    #[doc = include_str!("fmt_trait_method_doc.md")]
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Position {
    ///     longitude: f32,
    ///     latitude: f32,
    /// }
    ///
    /// impl fmt::Display for Position {
    ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         write!(f, "({}, {})", self.longitude, self.latitude)
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     "(1.987, 2.983)",
    ///     format!("{}", Position { longitude: 1.987, latitude: 2.983, }),
    /// );
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}

/// `o` 格式化。
///
/// `Octal` trait 应把输出格式化为八进制数。
///
/// 对原始有符号整数(`i8` 到 `i128` 以及 `isize`)而言,负值会按二进制补码表示格式化。
///
/// alternate flag `#` 会在输出前添加 `0o`。
///
/// 关于 formatter 的更多信息,见[模块级文档][module]。
///
/// [module]: ../../std/fmt/index.html
///
/// # 示例
///
/// `i32` 的基本用法:
///
/// ```
/// let x = 42; // 42 的八进制表示是 '52'
///
/// assert_eq!(format!("{x:o}"), "52");
/// assert_eq!(format!("{x:#o}"), "0o52");
///
/// assert_eq!(format!("{:o}", -16), "37777777760");
/// ```
///
/// 为某个类型实现 `Octal`:
///
/// ```
/// use std::fmt;
///
/// struct Length(i32);
///
/// impl fmt::Octal for Length {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         let val = self.0;
///
///         fmt::Octal::fmt(&val, f) // 委托给 i32 的实现
///     }
/// }
///
/// let l = Length(9);
///
/// assert_eq!(format!("l as octal is: {l:o}"), "l as octal is: 11");
///
/// assert_eq!(format!("l as octal is: {l:#06o}"), "l as octal is: 0o0011");
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub trait Octal: PointeeSized {
    #[doc = include_str!("fmt_trait_method_doc.md")]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}

/// `b` 格式化。
///
/// `Binary` trait 应把输出格式化为二进制数。
///
/// 对原始有符号整数([`i8`] 到 [`i128`] 以及 [`isize`])而言,负值会按二进制补码表示格式化。
///
/// alternate flag `#` 会在输出前添加 `0b`。
///
/// 关于 formatter 的更多信息,见[模块级文档][module]。
///
/// [module]: ../../std/fmt/index.html
///
/// # 示例
///
/// [`i32`] 的基本用法:
///
/// ```
/// let x = 42; // 42 的二进制表示是 '101010'
///
/// assert_eq!(format!("{x:b}"), "101010");
/// assert_eq!(format!("{x:#b}"), "0b101010");
///
/// assert_eq!(format!("{:b}", -16), "11111111111111111111111111110000");
/// ```
///
/// 为某个类型实现 `Binary`:
///
/// ```
/// use std::fmt;
///
/// struct Length(i32);
///
/// impl fmt::Binary for Length {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         let val = self.0;
///
///         fmt::Binary::fmt(&val, f) // 委托给 i32 的实现
///     }
/// }
///
/// let l = Length(107);
///
/// assert_eq!(format!("l as binary is: {l:b}"), "l as binary is: 1101011");
///
/// assert_eq!(
///     // 注意,`#` 添加的 `0b` 前缀会计入总宽度,因此需要额外加二,
///     // 才能正确显示全部 32 位。
///     format!("l as binary is: {l:#034b}"),
///     "l as binary is: 0b00000000000000000000000001101011"
/// );
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub trait Binary: PointeeSized {
    #[doc = include_str!("fmt_trait_method_doc.md")]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}

/// `x` 格式化。
///
/// `LowerHex` trait 应把输出格式化为十六进制数,并使用小写的 `a` 到 `f`。
///
/// 对原始有符号整数(`i8` 到 `i128` 以及 `isize`)而言,负值会按二进制补码表示格式化。
///
/// alternate flag `#` 会在输出前添加 `0x`。
///
/// 关于 formatter 的更多信息,见[模块级文档][module]。
///
/// [module]: ../../std/fmt/index.html
///
/// # 示例
///
/// `i32` 的基本用法:
///
/// ```
/// let y = 42; // 42 的十六进制表示是 '2a'
///
/// assert_eq!(format!("{y:x}"), "2a");
/// assert_eq!(format!("{y:#x}"), "0x2a");
///
/// assert_eq!(format!("{:x}", -16), "fffffff0");
/// ```
///
/// 为某个类型实现 `LowerHex`:
///
/// ```
/// use std::fmt;
///
/// struct Length(i32);
///
/// impl fmt::LowerHex for Length {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         let val = self.0;
///
///         fmt::LowerHex::fmt(&val, f) // 委托给 i32 的实现
///     }
/// }
///
/// let l = Length(9);
///
/// assert_eq!(format!("l as hex is: {l:x}"), "l as hex is: 9");
///
/// assert_eq!(format!("l as hex is: {l:#010x}"), "l as hex is: 0x00000009");
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub trait LowerHex: PointeeSized {
    #[doc = include_str!("fmt_trait_method_doc.md")]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}

/// `X` 格式化。
///
/// `UpperHex` trait 应把输出格式化为十六进制数,并使用大写的 `A` 到 `F`。
///
/// 对原始有符号整数(`i8` 到 `i128` 以及 `isize`)而言,负值会按二进制补码表示格式化。
///
/// alternate flag `#` 会在输出前添加 `0x`。
///
/// 关于 formatter 的更多信息,见[模块级文档][module]。
///
/// [module]: ../../std/fmt/index.html
///
/// # 示例
///
/// `i32` 的基本用法:
///
/// ```
/// let y = 42; // 42 的十六进制表示是 '2A'
///
/// assert_eq!(format!("{y:X}"), "2A");
/// assert_eq!(format!("{y:#X}"), "0x2A");
///
/// assert_eq!(format!("{:X}", -16), "FFFFFFF0");
/// ```
///
/// 为某个类型实现 `UpperHex`:
///
/// ```
/// use std::fmt;
///
/// struct Length(i32);
///
/// impl fmt::UpperHex for Length {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         let val = self.0;
///
///         fmt::UpperHex::fmt(&val, f) // 委托给 i32 的实现
///     }
/// }
///
/// let l = Length(i32::MAX);
///
/// assert_eq!(format!("l as hex is: {l:X}"), "l as hex is: 7FFFFFFF");
///
/// assert_eq!(format!("l as hex is: {l:#010X}"), "l as hex is: 0x7FFFFFFF");
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub trait UpperHex: PointeeSized {
    #[doc = include_str!("fmt_trait_method_doc.md")]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}

/// `p` 格式化。
///
/// `Pointer` trait 应把输出格式化为内存位置。它通常以十六进制呈现。
/// 关于 formatter 的更多信息,见[模块级文档][module]。
///
/// 打印指针并不是理解 Rust 程序实际实现方式的可靠方法。读取地址这一行为本身会改变程序,
/// 可能改变数据在内存中的表示,也可能影响编译器对代码应用哪些优化。
///
/// 打印出来的指针值既不保证稳定,也不保证是对象的唯一标识符。Rust 允许把值移动到
/// 不同内存位置,也允许为不同用途复用同一内存位置。
///
/// 不能保证打印出的值可以被转换回指针。
///
/// [module]: ../../std/fmt/index.html
///
/// # 示例
///
/// `&i32` 的基本用法:
///
/// ```
/// let x = &42;
///
/// let address = format!("{x:p}"); // 这会产生类似 '0x7f06092ac6d0' 的文本
/// ```
///
/// 为某个类型实现 `Pointer`:
///
/// ```
/// use std::fmt;
///
/// struct Length(i32);
///
/// impl fmt::Pointer for Length {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         // 使用 `as` 转换为 `*const T`;它实现了 Pointer,可供这里复用
///
///         let ptr = self as *const Self;
///         fmt::Pointer::fmt(&ptr, f)
///     }
/// }
///
/// let l = Length(42);
///
/// println!("l is in memory here: {l:p}");
///
/// let l_ptr = format!("{l:018p}");
/// assert_eq!(l_ptr.len(), 18);
/// assert_eq!(&l_ptr[..2], "0x");
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "Pointer"]
pub trait Pointer: PointeeSized {
    #[doc = include_str!("fmt_trait_method_doc.md")]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}

/// `e` 格式化。
///
/// `LowerExp` trait 应使用带小写 `e` 的科学计数法格式化输出。
///
/// 关于 formatter 的更多信息,见[模块级文档][module]。
///
/// [module]: ../../std/fmt/index.html
///
/// # 示例
///
/// `f64` 的基本用法:
///
/// ```
/// let x = 42.0; // 42.0 的科学计数法表示是 '4.2e1'
///
/// assert_eq!(format!("{x:e}"), "4.2e1");
/// ```
///
/// 为某个类型实现 `LowerExp`:
///
/// ```
/// use std::fmt;
///
/// struct Length(i32);
///
/// impl fmt::LowerExp for Length {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         let val = f64::from(self.0);
///         fmt::LowerExp::fmt(&val, f) // 委托给 f64 的实现
///     }
/// }
///
/// let l = Length(100);
///
/// assert_eq!(
///     format!("l in scientific notation is: {l:e}"),
///     "l in scientific notation is: 1e2"
/// );
///
/// assert_eq!(
///     format!("l in scientific notation is: {l:05e}"),
///     "l in scientific notation is: 001e2"
/// );
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub trait LowerExp: PointeeSized {
    #[doc = include_str!("fmt_trait_method_doc.md")]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}

/// `E` 格式化。
///
/// `UpperExp` trait 应使用带大写 `E` 的科学计数法格式化输出。
///
/// 关于 formatter 的更多信息,见[模块级文档][module]。
///
/// [module]: ../../std/fmt/index.html
///
/// # 示例
///
/// `f64` 的基本用法:
///
/// ```
/// let x = 42.0; // 42.0 的科学计数法表示是 '4.2E1'
///
/// assert_eq!(format!("{x:E}"), "4.2E1");
/// ```
///
/// 为某个类型实现 `UpperExp`:
///
/// ```
/// use std::fmt;
///
/// struct Length(i32);
///
/// impl fmt::UpperExp for Length {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         let val = f64::from(self.0);
///         fmt::UpperExp::fmt(&val, f) // 委托给 f64 的实现
///     }
/// }
///
/// let l = Length(100);
///
/// assert_eq!(
///     format!("l in scientific notation is: {l:E}"),
///     "l in scientific notation is: 1E2"
/// );
///
/// assert_eq!(
///     format!("l in scientific notation is: {l:05E}"),
///     "l in scientific notation is: 001E2"
/// );
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub trait UpperExp: PointeeSized {
    #[doc = include_str!("fmt_trait_method_doc.md")]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}

/// 接收一个输出流和一个可由 `format_args!` 宏预编译的 `Arguments` 结构。
///
/// 这些实参会按照指定格式字符串被格式化到提供的输出流中。该函数是 `write!`
/// 宏最终调用的核心入口之一,因此 panic 消息、日志和无分配格式化路径都会依赖它
/// 正确解释 `Arguments` 的模板编码。
///
/// # 示例
///
/// 基本用法:
///
/// ```
/// use std::fmt;
///
/// let mut output = String::new();
/// fmt::write(&mut output, format_args!("Hello {}!", "world"))
///     .expect("Error occurred while trying to write in String");
/// assert_eq!(output, "Hello world!");
/// ```
///
/// 请注意,使用 [`write!`] 可能更合适。示例:
///
/// ```
/// use std::fmt::Write;
///
/// let mut output = String::new();
/// write!(&mut output, "Hello {}!", "world")
///     .expect("Error occurred while trying to write in String");
/// assert_eq!(output, "Hello world!");
/// ```
///
/// [`write!`]: crate::write!
#[stable(feature = "rust1", since = "1.0.0")]
pub fn write(output: &mut dyn Write, fmt: Arguments<'_>) -> Result {
    if let Some(s) = fmt.as_str() {
        return output.write_str(s);
    }

    let mut template = fmt.template;
    let args = fmt.args;

    let mut arg_index = 0;

    // template 编码细节见 `fmt::Arguments` 上的注释。

    // 这里必须匹配 compiler/rustc_ast_lowering/src/format.rs 中
    // `expand_format_args` 生成的编码。
    loop {
        // SAFETY: 可以假定 template 合法。
        let n = unsafe {
            let n = template.read();
            template = template.add(1);
            n
        };

        if n == 0 {
            // template 结束。
            return Ok(());
        } else if n < 0x80 {
            // 长度为 `n` 的字面量字符串片段。

            // SAFETY: 可以假定 template 中的字符串合法。
            let s = unsafe {
                let s = crate::str::from_raw_parts(template.as_ptr(), n as usize);
                template = template.add(n as usize);
                s
            };
            output.write_str(s)?;
        } else if n == 0x80 {
            // 带 16 位长度的字面量字符串片段。

            // SAFETY: 可以假定 template 中的字符串合法。
            let s = unsafe {
                let len = usize::from(u16::from_le_bytes(template.cast_array().read()));
                template = template.add(2);
                let s = crate::str::from_raw_parts(template.as_ptr(), len);
                template = template.add(len);
                s
            };
            output.write_str(s)?;
        } else if n == 0xC0 {
            // 使用默认 options 的下一个参数占位符。
            //
            // 把它作为独立分支可以优化最常见情况的性能。

            // SAFETY: 可以假定 template 只引用实际存在的参数。
            unsafe {
                args.add(arg_index)
                    .as_ref()
                    .fmt(&mut Formatter::new(output, FormattingOptions::new()))?;
            }
            arg_index += 1;
        } else {
            // SAFETY: 可以假定 template 合法。
            unsafe { assert_unchecked(n > 0xC0) };

            // 带自定义 options 的占位符。

            let mut opt = FormattingOptions::new();

            // SAFETY: 可以假定 template 合法。
            unsafe {
                if n & 1 != 0 {
                    opt.flags = u32::from_le_bytes(template.cast_array().read());
                    template = template.add(4);
                }
                if n & 2 != 0 {
                    opt.width = u16::from_le_bytes(template.cast_array().read());
                    template = template.add(2);
                }
                if n & 4 != 0 {
                    opt.precision = u16::from_le_bytes(template.cast_array().read());
                    template = template.add(2);
                }
                if n & 8 != 0 {
                    arg_index = usize::from(u16::from_le_bytes(template.cast_array().read()));
                    template = template.add(2);
                }
            }
            if n & 16 != 0 {
                // 从 usize 参数读取动态 width。
                // SAFETY: 可以假定 template 只引用实际存在的参数。
                unsafe {
                    opt.width = args.add(opt.width as usize).as_ref().as_u16().unwrap_unchecked();
                }
            }
            if n & 32 != 0 {
                // 从 usize 参数读取动态 precision。
                // SAFETY: 可以假定 template 只引用实际存在的参数。
                unsafe {
                    opt.precision =
                        args.add(opt.precision as usize).as_ref().as_u16().unwrap_unchecked();
                }
            }

            // SAFETY: 可以假定 template 只引用实际存在的参数。
            unsafe {
                args.add(arg_index).as_ref().fmt(&mut Formatter::new(output, opt))?;
            }
            arg_index += 1;
        }
    }
}

/// 某个被填充对象之后的填充。由 `Formatter::padding` 返回。
#[must_use = "don't forget to write the post padding"]
pub(crate) struct PostPadding {
    fill: char,
    padding: u16,
}

impl PostPadding {
    fn new(fill: char, padding: u16) -> PostPadding {
        PostPadding { fill, padding }
    }

    /// 写入这段后置填充。
    pub(crate) fn write(self, f: &mut Formatter<'_>) -> Result {
        for _ in 0..self.padding {
            f.buf.write_char(self.fill)?;
        }
        Ok(())
    }
}

impl<'a> Formatter<'a> {
    fn wrap_buf<'b, 'c, F>(&'b mut self, wrap: F) -> Formatter<'c>
    where
        'b: 'c,
        F: FnOnce(&'b mut (dyn Write + 'b)) -> &'c mut (dyn Write + 'c),
    {
        Formatter {
            // 这里替换底层输出缓冲区。
            buf: wrap(self.buf),

            // 这里保留格式化选项。
            options: self.options,
        }
    }

    // 所有格式化 trait 都可复用的辅助方法,用于填充和处理格式化参数。

    /// 对已经写入 `str` 的整数文本执行正确填充。
    ///
    /// 传入的 `str` 不应包含整数符号;符号会由此方法根据 `Formatter` 的状态添加。
    ///
    /// # 参数
    ///
    /// * is_nonnegative - 原始整数是否为正数或零。
    /// * prefix - 若提供了 `#` 字符(Alternate),这是应放在数字前面的前缀。
    /// * buf - 已格式化出数字文本的字节数组。
    ///
    /// 此函数会正确考虑传入的 flags 和最小宽度,但不会考虑 precision。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo { nb: i32 }
    ///
    /// impl Foo {
    ///     fn new(nb: i32) -> Foo {
    ///         Foo {
    ///             nb,
    ///         }
    ///     }
    /// }
    ///
    /// impl fmt::Display for Foo {
    ///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         // 需要从数字输出中移除 "-"。
    ///         let tmp = self.nb.abs().to_string();
    ///
    ///         formatter.pad_integral(self.nb >= 0, "Foo ", &tmp)
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{}", Foo::new(2)), "2");
    /// assert_eq!(format!("{}", Foo::new(-1)), "-1");
    /// assert_eq!(format!("{}", Foo::new(0)), "0");
    /// assert_eq!(format!("{:#}", Foo::new(-1)), "-Foo 1");
    /// assert_eq!(format!("{:0>#8}", Foo::new(-1)), "00-Foo 1");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn pad_integral(&mut self, is_nonnegative: bool, prefix: &str, buf: &str) -> Result {
        let mut width = buf.len();

        let mut sign = None;
        if !is_nonnegative {
            sign = Some('-');
            width += 1;
        } else if self.sign_plus() {
            sign = Some('+');
            width += 1;
        }

        let prefix = if self.alternate() {
            width += prefix.chars().count();
            Some(prefix)
        } else {
            None
        };

        // 如果存在符号则先写入符号,随后按需写入前缀。
        #[inline(never)]
        fn write_prefix(f: &mut Formatter<'_>, sign: Option<char>, prefix: Option<&str>) -> Result {
            if let Some(c) = sign {
                f.buf.write_char(c)?;
            }
            if let Some(prefix) = prefix { f.buf.write_str(prefix) } else { Ok(()) }
        }

        // 到这里,`width` 字段更像是 `min-width` 参数。
        let min = self.options.width;
        if width >= usize::from(min) {
            // 已达到最小宽度,因此可以直接写入字节。
            write_prefix(self, sign, prefix)?;
            self.buf.write_str(buf)
        } else if self.sign_aware_zero_pad() {
            // 当填充字符为零时,符号和前缀位于填充之前。
            let old_options = self.options;
            self.options.fill('0').align(Some(Alignment::Right));
            write_prefix(self, sign, prefix)?;
            let post_padding = self.padding(min - width as u16, Alignment::Right)?;
            self.buf.write_str(buf)?;
            post_padding.write(self)?;
            self.options = old_options;
            Ok(())
        } else {
            // 否则,符号和前缀位于填充之后。
            let post_padding = self.padding(min - width as u16, Alignment::Right)?;
            write_prefix(self, sign, prefix)?;
            self.buf.write_str(buf)?;
            post_padding.write(self)
        }
    }

    /// 接收一个字符串切片,应用相关格式化 flags 后将其写入内部缓冲区。
    ///
    /// 泛型字符串会识别以下 flags:
    ///
    /// * width - 待输出内容的最小宽度。
    /// * fill/align - 当传入字符串需要填充时,填充什么以及填充到哪里。
    /// * precision - 待输出内容的最大长度;若字符串更长,会截断到该长度。
    ///
    /// 特别地,此函数会忽略 `flag` 参数。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo;
    ///
    /// impl fmt::Display for Foo {
    ///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         formatter.pad("Foo")
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{Foo:<4}"), "Foo ");
    /// assert_eq!(format!("{Foo:0>4}"), "0Foo");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn pad(&mut self, s: &str) -> Result {
        // 先保留一条快速路径。
        if self.options.flags & (flags::WIDTH_FLAG | flags::PRECISION_FLAG) == 0 {
            return self.buf.write_str(s);
        }

        // 对正在格式化的字符串而言,`precision` 字段可解释为最大宽度。
        let (s, char_count) = if let Some(max_char_count) = self.options.get_precision() {
            let mut iter = s.char_indices();
            let remaining = match iter.advance_by(usize::from(max_char_count)) {
                Ok(()) => 0,
                Err(remaining) => remaining.get(),
            };
            // SAFETY: `.char_indices()` 的 offset 保证在边界内且位于字符边界上。
            let truncated = unsafe { s.get_unchecked(..iter.offset()) };
            (truncated, usize::from(max_char_count) - remaining)
        } else {
            // 对完整字符串使用优化过的字符计数算法。
            (s, s.chars().count())
        };

        // 到这里,`width` 字段更像是最小宽度参数。
        if char_count < usize::from(self.options.width) {
            // 如果尚未达到最小宽度,则使用指定字符串加上某种 alignment 补足最小宽度。
            let post_padding =
                self.padding(self.options.width - char_count as u16, Alignment::Left)?;
            self.buf.write_str(s)?;
            post_padding.write(self)
        } else {
            // 如果已达到最小宽度,或者没有最小宽度,则可以直接输出字符串。
            self.buf.write_str(s)
        }
    }

    /// 写入前置填充,并返回尚未写入的后置填充。
    ///
    /// 调用方负责确保在被填充对象之后写入返回的后置填充。
    pub(crate) fn padding(
        &mut self,
        padding: u16,
        default: Alignment,
    ) -> result::Result<PostPadding, Error> {
        let align = self.options.get_align().unwrap_or(default);
        let fill = self.options.get_fill();

        let padding_left = match align {
            Alignment::Left => 0,
            Alignment::Right => padding,
            Alignment::Center => padding / 2,
        };

        for _ in 0..padding_left {
            self.buf.write_char(fill)?;
        }

        Ok(PostPadding::new(fill, padding - padding_left))
    }

    /// 接收已经拆分好的格式化片段并应用填充。
    ///
    /// 这里假定调用方已经按所需 precision 渲染好这些片段,因此可以忽略
    /// `self.precision`。
    ///
    /// # 安全性(Safety）
    ///
    /// `formatted` 中任何 `numfmt::Part::Copy` 片段都必须包含合法 UTF-8。
    unsafe fn pad_formatted_parts(&mut self, formatted: &numfmt::Formatted<'_>) -> Result {
        if self.options.width == 0 {
            // 这是常见情况,因此走快捷路径。
            // SAFETY: 根据前置条件。
            unsafe { self.write_formatted_parts(formatted) }
        } else {
            // 对 sign-aware zero padding,先渲染符号,之后表现得像一开始就没有符号。
            let mut formatted = formatted.clone();
            let mut width = self.options.width;
            let old_options = self.options;
            if self.sign_aware_zero_pad() {
                // 符号总是最先出现。
                let sign = formatted.sign;
                self.buf.write_str(sign)?;

                // 从 formatted parts 中移除符号。
                formatted.sign = "";
                width = width.saturating_sub(sign.len() as u16);
                self.options.fill('0').align(Some(Alignment::Right));
            }

            // 剩余片段走普通填充流程。
            let len = formatted.len();
            let ret = if usize::from(width) <= len {
                // 不需要填充。
                // SAFETY: 根据前置条件。
                unsafe { self.write_formatted_parts(&formatted) }
            } else {
                let post_padding = self.padding(width - len as u16, Alignment::Right)?;
                // SAFETY: 根据前置条件。
                unsafe {
                    self.write_formatted_parts(&formatted)?;
                }
                post_padding.write(self)
            };
            self.options = old_options;
            ret
        }
    }

    /// # 安全性(Safety）
    ///
    /// `formatted` 中任何 `numfmt::Part::Copy` 片段都必须包含合法 UTF-8。
    unsafe fn write_formatted_parts(&mut self, formatted: &numfmt::Formatted<'_>) -> Result {
        unsafe fn write_bytes(buf: &mut dyn Write, s: &[u8]) -> Result {
            // SAFETY: 此函数用于 `numfmt::Part::Num` 和 `numfmt::Part::Copy`。
            // 对 `numfmt::Part::Num` 是安全的,因为每个字符 `c` 都位于 `b'0'`
            // 到 `b'9'` 之间,这意味着 `s` 是合法 UTF-8。对 `numfmt::Part::Copy`
            // 是安全的,因为本函数有相应前置条件。
            buf.write_str(unsafe { str::from_utf8_unchecked(s) })
        }

        if !formatted.sign.is_empty() {
            self.buf.write_str(formatted.sign)?;
        }
        for part in formatted.parts {
            match *part {
                numfmt::Part::Zero(mut nzeroes) => {
                    const ZEROES: &str = // 64 个零
                        "0000000000000000000000000000000000000000000000000000000000000000";
                    while nzeroes > ZEROES.len() {
                        self.buf.write_str(ZEROES)?;
                        nzeroes -= ZEROES.len();
                    }
                    if nzeroes > 0 {
                        self.buf.write_str(&ZEROES[..nzeroes])?;
                    }
                }
                numfmt::Part::Num(mut v) => {
                    let mut s = [0; 5];
                    let len = part.len();
                    for c in s[..len].iter_mut().rev() {
                        *c = b'0' + (v % 10) as u8;
                        v /= 10;
                    }
                    // SAFETY: 根据前置条件。
                    unsafe {
                        write_bytes(self.buf, &s[..len])?;
                    }
                }
                // SAFETY: 根据前置条件。
                numfmt::Part::Copy(buf) => unsafe {
                    write_bytes(self.buf, buf)?;
                },
            }
        }
        Ok(())
    }

    /// 向此 formatter 包含的底层缓冲区写入一些数据。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo;
    ///
    /// impl fmt::Display for Foo {
    ///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         formatter.write_str("Foo")
    ///         // 这等价于:
    ///         // write!(formatter, "Foo")
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{Foo}"), "Foo");
    /// assert_eq!(format!("{Foo:0>8}"), "Foo");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn write_str(&mut self, data: &str) -> Result {
        self.buf.write_str(data)
    }

    /// 供 [`write!`] 宏配合此 trait 的实现者使用的衔接方法。
    ///
    /// 通常不应手动调用此方法,而应通过 [`write!`] 宏调用。
    ///
    /// 向此实例写入一些已格式化的信息。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(i32);
    ///
    /// impl fmt::Display for Foo {
    ///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         formatter.write_fmt(format_args!("Foo {}", self.0))
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{}", Foo(-1)), "Foo -1");
    /// assert_eq!(format!("{:0>8}", Foo(2)), "Foo 2");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn write_fmt(&mut self, fmt: Arguments<'_>) -> Result {
        if let Some(s) = fmt.as_statically_known_str() {
            self.buf.write_str(s)
        } else {
            write(self.buf, fmt)
        }
    }

    /// 返回格式化 flags。
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[deprecated(
        since = "1.24.0",
        note = "use the `sign_plus`, `sign_minus`, `alternate`, \
                or `sign_aware_zero_pad` methods instead"
    )]
    pub fn flags(&self) -> u32 {
        // 提取 Debug 大/小写十六进制、补零、alternate 和正负号 flags,
        // 以保持与旧 Rust 版本兼容。
        self.options.flags >> 21 & 0x3F
    }

    /// 当存在 alignment 时,返回用作 fill 的字符。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo;
    ///
    /// impl fmt::Display for Foo {
    ///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         let c = formatter.fill();
    ///         if let Some(width) = formatter.width() {
    ///             for _ in 0..width {
    ///                 write!(formatter, "{c}")?;
    ///             }
    ///             Ok(())
    ///         } else {
    ///             write!(formatter, "{c}")
    ///         }
    ///     }
    /// }
    ///
    /// // 使用 ">" 把 alignment 设置为右对齐。
    /// assert_eq!(format!("{Foo:G>3}"), "GGG");
    /// assert_eq!(format!("{Foo:t>6}"), "tttttt");
    /// ```
    #[must_use]
    #[stable(feature = "fmt_flags", since = "1.5.0")]
    pub fn fill(&self) -> char {
        self.options.get_fill()
    }

    /// 返回一个 flag,表示请求了哪种 alignment。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt::{self, Alignment};
    ///
    /// struct Foo;
    ///
    /// impl fmt::Display for Foo {
    ///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         let s = if let Some(s) = formatter.align() {
    ///             match s {
    ///                 Alignment::Left    => "left",
    ///                 Alignment::Right   => "right",
    ///                 Alignment::Center  => "center",
    ///             }
    ///         } else {
    ///             "into the void"
    ///         };
    ///         write!(formatter, "{s}")
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{Foo:<}"), "left");
    /// assert_eq!(format!("{Foo:>}"), "right");
    /// assert_eq!(format!("{Foo:^}"), "center");
    /// assert_eq!(format!("{Foo}"), "into the void");
    /// ```
    #[must_use]
    #[stable(feature = "fmt_flags_align", since = "1.28.0")]
    pub fn align(&self) -> Option<Alignment> {
        self.options.get_align()
    }

    /// 返回输出应使用的可选整数 width。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(i32);
    ///
    /// impl fmt::Display for Foo {
    ///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         if let Some(width) = formatter.width() {
    ///             // 如果收到 width,就使用它。
    ///             write!(formatter, "{:width$}", format!("Foo({})", self.0), width = width)
    ///         } else {
    ///             // 否则不做特殊处理。
    ///             write!(formatter, "Foo({})", self.0)
    ///         }
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{:10}", Foo(23)), "Foo(23)   ");
    /// assert_eq!(format!("{}", Foo(23)), "Foo(23)");
    /// ```
    #[must_use]
    #[stable(feature = "fmt_flags", since = "1.5.0")]
    pub fn width(&self) -> Option<usize> {
        if self.options.flags & flags::WIDTH_FLAG == 0 {
            None
        } else {
            Some(self.options.width as usize)
        }
    }

    /// 返回数值类型可选指定的 precision。
    /// 对字符串类型,它也可以表示最大宽度。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(f32);
    ///
    /// impl fmt::Display for Foo {
    ///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         if let Some(precision) = formatter.precision() {
    ///             // 如果收到 precision,就使用它。
    ///             write!(formatter, "Foo({1:.*})", precision, self.0)
    ///         } else {
    ///             // 否则默认使用 2。
    ///             write!(formatter, "Foo({:.2})", self.0)
    ///         }
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{:.4}", Foo(23.2)), "Foo(23.2000)");
    /// assert_eq!(format!("{}", Foo(23.2)), "Foo(23.20)");
    /// ```
    #[must_use]
    #[stable(feature = "fmt_flags", since = "1.5.0")]
    pub fn precision(&self) -> Option<usize> {
        if self.options.flags & flags::PRECISION_FLAG == 0 {
            None
        } else {
            Some(self.options.precision as usize)
        }
    }

    /// 判断是否指定了 `+` flag。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(i32);
    ///
    /// impl fmt::Display for Foo {
    ///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         if formatter.sign_plus() {
    ///             write!(formatter,
    ///                    "Foo({}{})",
    ///                    if self.0 < 0 { '-' } else { '+' },
    ///                    self.0.abs())
    ///         } else {
    ///             write!(formatter, "Foo({})", self.0)
    ///         }
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{:+}", Foo(23)), "Foo(+23)");
    /// assert_eq!(format!("{:+}", Foo(-23)), "Foo(-23)");
    /// assert_eq!(format!("{}", Foo(23)), "Foo(23)");
    /// ```
    #[must_use]
    #[stable(feature = "fmt_flags", since = "1.5.0")]
    pub fn sign_plus(&self) -> bool {
        self.options.flags & flags::SIGN_PLUS_FLAG != 0
    }

    /// 判断是否指定了 `-` flag。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(i32);
    ///
    /// impl fmt::Display for Foo {
    ///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         if formatter.sign_minus() {
    ///             // 想要负号?这里给一个。
    ///             write!(formatter, "-Foo({})", self.0)
    ///         } else {
    ///             write!(formatter, "Foo({})", self.0)
    ///         }
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{:-}", Foo(23)), "-Foo(23)");
    /// assert_eq!(format!("{}", Foo(23)), "Foo(23)");
    /// ```
    #[must_use]
    #[stable(feature = "fmt_flags", since = "1.5.0")]
    pub fn sign_minus(&self) -> bool {
        self.options.flags & flags::SIGN_MINUS_FLAG != 0
    }

    /// 判断是否指定了 `#` flag。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(i32);
    ///
    /// impl fmt::Display for Foo {
    ///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         if formatter.alternate() {
    ///             write!(formatter, "Foo({})", self.0)
    ///         } else {
    ///             write!(formatter, "{}", self.0)
    ///         }
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{:#}", Foo(23)), "Foo(23)");
    /// assert_eq!(format!("{}", Foo(23)), "23");
    /// ```
    #[must_use]
    #[stable(feature = "fmt_flags", since = "1.5.0")]
    pub fn alternate(&self) -> bool {
        self.options.flags & flags::ALTERNATE_FLAG != 0
    }

    /// 判断是否指定了 `0` flag。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(i32);
    ///
    /// impl fmt::Display for Foo {
    ///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         assert!(formatter.sign_aware_zero_pad());
    ///         assert_eq!(formatter.width(), Some(4));
    ///         // 这里忽略 formatter 的 options。
    ///         write!(formatter, "{}", self.0)
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{:04}", Foo(23)), "23");
    /// ```
    #[must_use]
    #[stable(feature = "fmt_flags", since = "1.5.0")]
    pub fn sign_aware_zero_pad(&self) -> bool {
        self.options.flags & flags::SIGN_AWARE_ZERO_PAD_FLAG != 0
    }

    // FIXME: 决定这两个 flags 应采用什么公开 API。
    // https://github.com/rust-lang/rust/issues/48584
    fn debug_lower_hex(&self) -> bool {
        self.options.flags & flags::DEBUG_LOWER_HEX_FLAG != 0
    }
    fn debug_upper_hex(&self) -> bool {
        self.options.flags & flags::DEBUG_UPPER_HEX_FLAG != 0
    }

    /// 创建一个 [`DebugStruct`] builder,用于辅助为结构体编写 [`fmt::Debug`] 实现。
    ///
    /// [`fmt::Debug`]: self::Debug
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::fmt;
    /// use std::net::Ipv4Addr;
    ///
    /// struct Foo {
    ///     bar: i32,
    ///     baz: String,
    ///     addr: Ipv4Addr,
    /// }
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_struct("Foo")
    ///             .field("bar", &self.bar)
    ///             .field("baz", &self.baz)
    ///             .field("addr", &format_args!("{}", self.addr))
    ///             .finish()
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     "Foo { bar: 10, baz: \"Hello World\", addr: 127.0.0.1 }",
    ///     format!("{:?}", Foo {
    ///         bar: 10,
    ///         baz: "Hello World".to_string(),
    ///         addr: Ipv4Addr::new(127, 0, 0, 1),
    ///     })
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn debug_struct<'b>(&'b mut self, name: &str) -> DebugStruct<'b, 'a> {
        builders::debug_struct_new(self, name)
    }

    /// 缩小 `derive(Debug)` 生成的代码,以加快编译并减小二进制体积。
    /// `debug_struct_fields_finish` 更通用,但此方法在 1 个字段时更快。
    #[doc(hidden)]
    #[unstable(feature = "fmt_helpers_for_derive", issue = "none")]
    pub fn debug_struct_field1_finish<'b>(
        &'b mut self,
        name: &str,
        name1: &str,
        value1: &dyn Debug,
    ) -> Result {
        let mut builder = builders::debug_struct_new(self, name);
        builder.field(name1, value1);
        builder.finish()
    }

    /// 缩小 `derive(Debug)` 生成的代码,以加快编译并减小二进制体积。
    /// `debug_struct_fields_finish` 更通用,但此方法在 2 个字段时更快。
    #[doc(hidden)]
    #[unstable(feature = "fmt_helpers_for_derive", issue = "none")]
    pub fn debug_struct_field2_finish<'b>(
        &'b mut self,
        name: &str,
        name1: &str,
        value1: &dyn Debug,
        name2: &str,
        value2: &dyn Debug,
    ) -> Result {
        let mut builder = builders::debug_struct_new(self, name);
        builder.field(name1, value1);
        builder.field(name2, value2);
        builder.finish()
    }

    /// 缩小 `derive(Debug)` 生成的代码,以加快编译并减小二进制体积。
    /// `debug_struct_fields_finish` 更通用,但此方法在 3 个字段时更快。
    #[doc(hidden)]
    #[unstable(feature = "fmt_helpers_for_derive", issue = "none")]
    pub fn debug_struct_field3_finish<'b>(
        &'b mut self,
        name: &str,
        name1: &str,
        value1: &dyn Debug,
        name2: &str,
        value2: &dyn Debug,
        name3: &str,
        value3: &dyn Debug,
    ) -> Result {
        let mut builder = builders::debug_struct_new(self, name);
        builder.field(name1, value1);
        builder.field(name2, value2);
        builder.field(name3, value3);
        builder.finish()
    }

    /// 缩小 `derive(Debug)` 生成的代码,以加快编译并减小二进制体积。
    /// `debug_struct_fields_finish` 更通用,但此方法在 4 个字段时更快。
    #[doc(hidden)]
    #[unstable(feature = "fmt_helpers_for_derive", issue = "none")]
    pub fn debug_struct_field4_finish<'b>(
        &'b mut self,
        name: &str,
        name1: &str,
        value1: &dyn Debug,
        name2: &str,
        value2: &dyn Debug,
        name3: &str,
        value3: &dyn Debug,
        name4: &str,
        value4: &dyn Debug,
    ) -> Result {
        let mut builder = builders::debug_struct_new(self, name);
        builder.field(name1, value1);
        builder.field(name2, value2);
        builder.field(name3, value3);
        builder.field(name4, value4);
        builder.finish()
    }

    /// 缩小 `derive(Debug)` 生成的代码,以加快编译并减小二进制体积。
    /// `debug_struct_fields_finish` 更通用,但此方法在 5 个字段时更快。
    #[doc(hidden)]
    #[unstable(feature = "fmt_helpers_for_derive", issue = "none")]
    pub fn debug_struct_field5_finish<'b>(
        &'b mut self,
        name: &str,
        name1: &str,
        value1: &dyn Debug,
        name2: &str,
        value2: &dyn Debug,
        name3: &str,
        value3: &dyn Debug,
        name4: &str,
        value4: &dyn Debug,
        name5: &str,
        value5: &dyn Debug,
    ) -> Result {
        let mut builder = builders::debug_struct_new(self, name);
        builder.field(name1, value1);
        builder.field(name2, value2);
        builder.field(name3, value3);
        builder.field(name4, value4);
        builder.field(name5, value5);
        builder.finish()
    }

    /// 缩小 `derive(Debug)` 生成的代码,以加快编译并减小二进制体积。
    /// 用于 `debug_struct_field[12345]_finish` 未覆盖的情况。
    #[doc(hidden)]
    #[unstable(feature = "fmt_helpers_for_derive", issue = "none")]
    pub fn debug_struct_fields_finish<'b>(
        &'b mut self,
        name: &str,
        names: &[&str],
        values: &[&dyn Debug],
    ) -> Result {
        assert_eq!(names.len(), values.len());
        let mut builder = builders::debug_struct_new(self, name);
        for (name, value) in iter::zip(names, values) {
            builder.field(name, value);
        }
        builder.finish()
    }

    /// 创建一个 `DebugTuple` builder,用于辅助为元组结构体编写 `fmt::Debug` 实现。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::fmt;
    /// use std::marker::PhantomData;
    ///
    /// struct Foo<T>(i32, String, PhantomData<T>);
    ///
    /// impl<T> fmt::Debug for Foo<T> {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_tuple("Foo")
    ///             .field(&self.0)
    ///             .field(&self.1)
    ///             .field(&format_args!("_"))
    ///             .finish()
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     "Foo(10, \"Hello\", _)",
    ///     format!("{:?}", Foo(10, "Hello".to_string(), PhantomData::<u8>))
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn debug_tuple<'b>(&'b mut self, name: &str) -> DebugTuple<'b, 'a> {
        builders::debug_tuple_new(self, name)
    }

    /// 缩小 `derive(Debug)` 生成的代码,以加快编译并减小二进制体积。
    /// `debug_tuple_fields_finish` 更通用,但此方法在 1 个字段时更快。
    #[doc(hidden)]
    #[unstable(feature = "fmt_helpers_for_derive", issue = "none")]
    pub fn debug_tuple_field1_finish<'b>(&'b mut self, name: &str, value1: &dyn Debug) -> Result {
        let mut builder = builders::debug_tuple_new(self, name);
        builder.field(value1);
        builder.finish()
    }

    /// 缩小 `derive(Debug)` 生成的代码,以加快编译并减小二进制体积。
    /// `debug_tuple_fields_finish` 更通用,但此方法在 2 个字段时更快。
    #[doc(hidden)]
    #[unstable(feature = "fmt_helpers_for_derive", issue = "none")]
    pub fn debug_tuple_field2_finish<'b>(
        &'b mut self,
        name: &str,
        value1: &dyn Debug,
        value2: &dyn Debug,
    ) -> Result {
        let mut builder = builders::debug_tuple_new(self, name);
        builder.field(value1);
        builder.field(value2);
        builder.finish()
    }

    /// 缩小 `derive(Debug)` 生成的代码,以加快编译并减小二进制体积。
    /// `debug_tuple_fields_finish` 更通用,但此方法在 3 个字段时更快。
    #[doc(hidden)]
    #[unstable(feature = "fmt_helpers_for_derive", issue = "none")]
    pub fn debug_tuple_field3_finish<'b>(
        &'b mut self,
        name: &str,
        value1: &dyn Debug,
        value2: &dyn Debug,
        value3: &dyn Debug,
    ) -> Result {
        let mut builder = builders::debug_tuple_new(self, name);
        builder.field(value1);
        builder.field(value2);
        builder.field(value3);
        builder.finish()
    }

    /// 缩小 `derive(Debug)` 生成的代码,以加快编译并减小二进制体积。
    /// `debug_tuple_fields_finish` 更通用,但此方法在 4 个字段时更快。
    #[doc(hidden)]
    #[unstable(feature = "fmt_helpers_for_derive", issue = "none")]
    pub fn debug_tuple_field4_finish<'b>(
        &'b mut self,
        name: &str,
        value1: &dyn Debug,
        value2: &dyn Debug,
        value3: &dyn Debug,
        value4: &dyn Debug,
    ) -> Result {
        let mut builder = builders::debug_tuple_new(self, name);
        builder.field(value1);
        builder.field(value2);
        builder.field(value3);
        builder.field(value4);
        builder.finish()
    }

    /// 缩小 `derive(Debug)` 生成的代码,以加快编译并减小二进制体积。
    /// `debug_tuple_fields_finish` 更通用,但此方法在 5 个字段时更快。
    #[doc(hidden)]
    #[unstable(feature = "fmt_helpers_for_derive", issue = "none")]
    pub fn debug_tuple_field5_finish<'b>(
        &'b mut self,
        name: &str,
        value1: &dyn Debug,
        value2: &dyn Debug,
        value3: &dyn Debug,
        value4: &dyn Debug,
        value5: &dyn Debug,
    ) -> Result {
        let mut builder = builders::debug_tuple_new(self, name);
        builder.field(value1);
        builder.field(value2);
        builder.field(value3);
        builder.field(value4);
        builder.field(value5);
        builder.finish()
    }

    /// 缩小 `derive(Debug)` 生成的代码,以加快编译并减小二进制体积。
    /// 用于 `debug_tuple_field[12345]_finish` 未覆盖的情况。
    #[doc(hidden)]
    #[unstable(feature = "fmt_helpers_for_derive", issue = "none")]
    pub fn debug_tuple_fields_finish<'b>(
        &'b mut self,
        name: &str,
        values: &[&dyn Debug],
    ) -> Result {
        let mut builder = builders::debug_tuple_new(self, name);
        for value in values {
            builder.field(value);
        }
        builder.finish()
    }

    /// 创建一个 `DebugList` builder,用于辅助为类列表结构编写 `fmt::Debug` 实现。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::fmt;
    ///
    /// struct Foo(Vec<i32>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_list().entries(self.0.iter()).finish()
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{:?}", Foo(vec![10, 11])), "[10, 11]");
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn debug_list<'b>(&'b mut self) -> DebugList<'b, 'a> {
        builders::debug_list_new(self)
    }

    /// 创建一个 `DebugSet` builder,用于辅助为类集合结构编写 `fmt::Debug` 实现。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::fmt;
    ///
    /// struct Foo(Vec<i32>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_set().entries(self.0.iter()).finish()
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{:?}", Foo(vec![10, 11])), "{10, 11}");
    /// ```
    ///
    /// [`format_args!`]: crate::format_args
    ///
    /// 在这个更复杂的示例中,我们使用 [`format_args!`] 和 `.debug_set()`
    /// 构造 match arms 列表:
    ///
    /// ```rust
    /// use std::fmt;
    ///
    /// struct Arm<'a, L, R>(&'a (L, R));
    /// struct Table<'a, K, V>(&'a [(K, V)], V);
    ///
    /// impl<'a, L, R> fmt::Debug for Arm<'a, L, R>
    /// where
    ///     L: 'a + fmt::Debug, R: 'a + fmt::Debug
    /// {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         L::fmt(&(self.0).0, fmt)?;
    ///         fmt.write_str(" => ")?;
    ///         R::fmt(&(self.0).1, fmt)
    ///     }
    /// }
    ///
    /// impl<'a, K, V> fmt::Debug for Table<'a, K, V>
    /// where
    ///     K: 'a + fmt::Debug, V: 'a + fmt::Debug
    /// {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_set()
    ///         .entries(self.0.iter().map(Arm))
    ///         .entry(&Arm(&(format_args!("_"), &self.1)))
    ///         .finish()
    ///     }
    /// }
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn debug_set<'b>(&'b mut self) -> DebugSet<'b, 'a> {
        builders::debug_set_new(self)
    }

    /// 创建一个 `DebugMap` builder,用于辅助为类映射结构编写 `fmt::Debug` 实现。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::fmt;
    ///
    /// struct Foo(Vec<(String, i32)>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_map().entries(self.0.iter().map(|&(ref k, ref v)| (k, v))).finish()
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}",  Foo(vec![("A".to_string(), 10), ("B".to_string(), 11)])),
    ///     r#"{"A": 10, "B": 11}"#
    ///  );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn debug_map<'b>(&'b mut self) -> DebugMap<'b, 'a> {
        builders::debug_map_new(self)
    }

    /// 返回此 formatter 的符号设置(`+` 或 `-`)。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn sign(&self) -> Option<Sign> {
        self.options.get_sign()
    }

    /// 返回此 formatter 对应的格式化选项。
    #[unstable(feature = "formatting_options", issue = "118117")]
    pub const fn options(&self) -> FormattingOptions {
        self.options
    }
}

#[stable(since = "1.2.0", feature = "formatter_write")]
impl Write for Formatter<'_> {
    fn write_str(&mut self, s: &str) -> Result {
        self.buf.write_str(s)
    }

    fn write_char(&mut self, c: char) -> Result {
        self.buf.write_char(c)
    }

    #[inline]
    fn write_fmt(&mut self, args: Arguments<'_>) -> Result {
        if let Some(s) = args.as_statically_known_str() {
            self.buf.write_str(s)
        } else {
            write(self.buf, args)
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Display::fmt("an error occurred when formatting an argument", f)
    }
}

// 核心格式化 trait 的实现

macro_rules! fmt_refs {
    ($($tr:ident),*) => {
        $(
        #[stable(feature = "rust1", since = "1.0.0")]
        impl<T: PointeeSized + $tr> $tr for &T {
            fn fmt(&self, f: &mut Formatter<'_>) -> Result { $tr::fmt(&**self, f) }
        }
        #[stable(feature = "rust1", since = "1.0.0")]
        impl<T: PointeeSized + $tr> $tr for &mut T {
            fn fmt(&self, f: &mut Formatter<'_>) -> Result { $tr::fmt(&**self, f) }
        }
        )*
    }
}

fmt_refs! { Debug, Display, Octal, Binary, LowerHex, UpperHex, LowerExp, UpperExp }

#[unstable(feature = "never_type", issue = "35121")]
impl Debug for ! {
    #[inline]
    fn fmt(&self, _: &mut Formatter<'_>) -> Result {
        *self
    }
}

#[unstable(feature = "never_type", issue = "35121")]
impl Display for ! {
    #[inline]
    fn fmt(&self, _: &mut Formatter<'_>) -> Result {
        *self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Debug for bool {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Display::fmt(self, f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Display for bool {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Display::fmt(if *self { "true" } else { "false" }, f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Debug for str {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_char('"')?;

        // 已知可打印的子串范围。
        let mut printable_range = 0..0;

        fn needs_escape(b: u8) -> bool {
            b > 0x7E || b < 0x20 || b == b'\\' || b == b'"'
        }

        // 这里的循环先把连续可打印 ASCII 作为快速路径跳过。
        // 其他字符(Unicode,或需要转义的 ASCII)再逐个 `char` 处理。
        let mut rest = self;
        while rest.len() > 0 {
            let Some(non_printable_start) = rest.as_bytes().iter().position(|&b| needs_escape(b))
            else {
                printable_range.end += rest.len();
                break;
            };

            printable_range.end += non_printable_start;
            // SAFETY: 该位置来自迭代器,因此已知在边界内且位于字符边界上。
            rest = unsafe { rest.get_unchecked(non_printable_start..) };

            let mut chars = rest.chars();
            if let Some(c) = chars.next() {
                let esc = c.escape_debug_ext(EscapeDebugExtArgs {
                    escape_grapheme_extended: true,
                    escape_single_quote: false,
                    escape_double_quote: true,
                });
                if esc.len() != 1 {
                    f.write_str(&self[printable_range.clone()])?;
                    Display::fmt(&esc, f)?;
                    printable_range.start = printable_range.end + c.len_utf8();
                }
                printable_range.end += c.len_utf8();
            }
            rest = chars.as_str();
        }

        f.write_str(&self[printable_range])?;

        f.write_char('"')
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Display for str {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.pad(self)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Debug for char {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_char('\'')?;
        let esc = self.escape_debug_ext(EscapeDebugExtArgs {
            escape_grapheme_extended: true,
            escape_single_quote: true,
            escape_double_quote: false,
        });
        Display::fmt(&esc, f)?;
        f.write_char('\'')
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Display for char {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        if f.options.flags & (flags::WIDTH_FLAG | flags::PRECISION_FLAG) == 0 {
            f.write_char(*self)
        } else {
            f.pad(self.encode_utf8(&mut [0; char::MAX_LEN_UTF8]))
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> Pointer for *const T {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        if <<T as core::ptr::Pointee>::Metadata as core::unit::IsUnit>::is_unit() {
            pointer_fmt_inner(self.expose_provenance(), f)
        } else {
            f.debug_struct("Pointer")
                .field_with("addr", |f| pointer_fmt_inner(self.expose_provenance(), f))
                .field("metadata", &core::ptr::metadata(*self))
                .finish()
        }
    }
}

/// 由于所有指针类型的格式化结果相同,实际格式化使用非单态化实现,
/// 以减少所需的 codegen 工作量。
///
/// 这里使用 `ptr_addr: usize` 而不是 `ptr: *const ()`,是为了让此函数也能用于
/// `fn(...) -> ...`,且无需使用有问题的 "Oxford Casts"。
///
/// [problematic]: https://github.com/rust-lang/rust/issues/95489
pub(crate) fn pointer_fmt_inner(ptr_addr: usize, f: &mut Formatter<'_>) -> Result {
    let old_options = f.options;

    // LowerHex 已经把 alternate flag 当作特殊情况处理:它表示是否加上 0x 前缀。
    // 我们用它判断是否需要补零扩展,随后无条件设置它以取得前缀。
    if f.options.get_alternate() {
        f.options.sign_aware_zero_pad(true);

        if f.options.get_width().is_none() {
            f.options.width(Some((usize::BITS / 4) as u16 + 2));
        }
    }
    f.options.alternate(true);

    let ret = LowerHex::fmt(&ptr_addr, f);

    f.options = old_options;

    ret
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> Pointer for *mut T {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Pointer::fmt(&(*self as *const T), f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> Pointer for &T {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Pointer::fmt(&(*self as *const T), f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> Pointer for &mut T {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Pointer::fmt(&(&**self as *const T), f)
    }
}

// 各种 core 类型的 Display/Debug 实现

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> Debug for *const T {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Pointer::fmt(self, f)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> Debug for *mut T {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Pointer::fmt(self, f)
    }
}

macro_rules! peel {
    ($name:ident, $($other:ident,)*) => (tuple! { $($other,)* })
}

macro_rules! tuple {
    () => ();
    ( $($name:ident,)+ ) => (
        maybe_tuple_doc! {
            $($name)+ @
            #[stable(feature = "rust1", since = "1.0.0")]
            impl<$($name:Debug),+> Debug for ($($name,)+) {
                #[allow(non_snake_case, unused_assignments)]
                fn fmt(&self, f: &mut Formatter<'_>) -> Result {
                    let mut builder = f.debug_tuple("");
                    let ($(ref $name,)+) = *self;
                    $(
                        builder.field(&$name);
                    )+

                    builder.finish()
                }
            }
        }
        peel! { $($name,)+ }
    )
}

macro_rules! maybe_tuple_doc {
    ($a:ident @ #[$meta:meta] $item:item) => {
        #[doc(fake_variadic)]
        #[doc = "This trait is implemented for tuples up to twelve items long."]
        #[$meta]
        $item
    };
    ($a:ident $($rest_a:ident)+ @ #[$meta:meta] $item:item) => {
        #[doc(hidden)]
        #[$meta]
        $item
    };
}

tuple! { E, D, C, B, A, Z, Y, X, W, V, U, T, }

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: Debug> Debug for [T] {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Debug for () {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.pad("()")
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> Debug for PhantomData<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "PhantomData<{}>", crate::any::type_name::<T>())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: Copy + Debug> Debug for Cell<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Cell").field("value", &self.get()).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized + Debug> Debug for RefCell<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let mut d = f.debug_struct("RefCell");
        match self.try_borrow() {
            Ok(borrow) => d.field("value", &borrow),
            Err(_) => d.field("value", &format_args!("<borrowed>")),
        };
        d.finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized + Debug> Debug for Ref<'_, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Debug::fmt(&**self, f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized + Debug> Debug for RefMut<'_, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Debug::fmt(&*(self.deref()), f)
    }
}

#[stable(feature = "core_impl_debug", since = "1.9.0")]
impl<T: ?Sized> Debug for UnsafeCell<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("UnsafeCell").finish_non_exhaustive()
    }
}

#[unstable(feature = "sync_unsafe_cell", issue = "95439")]
impl<T: ?Sized> Debug for SyncUnsafeCell<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("SyncUnsafeCell").finish_non_exhaustive()
    }
}

// 如果你原本预期测试在这里,请改看 coretests/tests/fmt/;
// 那比在这里构造所有 rt::Piece 结构容易得多。
// 对需要分配的场景,alloctests/tests/fmt.rs 中也有测试。
