//! 浮点和整数格式化共用的内部工具。
//!
//! `core` 不能依赖堆分配，因此这里把格式化结果拆成若干可直接写入字节缓冲区的片段。
//! 调用方可以先精确计算输出长度，再把符号、补零和数字片段顺序写入目标缓冲区；
//! 如果缓冲区不足，函数返回 `None`，但实现允许留下部分写入的字节，调用方不能依赖
//! 这些中间内容。
#![doc(hidden)]
#![unstable(
    feature = "numfmt",
    reason = "internal routines only exposed for testing",
    issue = "none"
)]

/// 一个格式化结果中的片段。
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Part<'a> {
    /// 指定数量的 ASCII 零字符。
    Zero(usize),
    /// 最多 5 位的十进制字面数字。
    Num(u16),
    /// 原样复制给定的字节序列。
    Copy(&'a [u8]),
}

impl<'a> Part<'a> {
    /// 返回该片段写出后的精确字节长度。
    pub fn len(&self) -> usize {
        match *self {
            Part::Zero(nzeroes) => nzeroes,
            Part::Num(v) => v.checked_ilog10().unwrap_or_default() as usize + 1,
            Part::Copy(buf) => buf.len(),
        }
    }

    /// 把该片段写入提供的缓冲区。
    ///
    /// 成功时返回写入的字节数；如果缓冲区不足则返回 `None`。失败路径可能已经写入了
    /// 一部分字节，这是内部格式化例程为避免额外临时缓冲作出的取舍，调用方必须把
    /// 返回值作为唯一可信的完成信号。
    pub fn write(&self, out: &mut [u8]) -> Option<usize> {
        let len = self.len();
        if out.len() >= len {
            match *self {
                Part::Zero(nzeroes) => {
                    for c in &mut out[..nzeroes] {
                        *c = b'0';
                    }
                }
                Part::Num(mut v) => {
                    for c in out[..len].iter_mut().rev() {
                        *c = b'0' + (v % 10) as u8;
                        v /= 10;
                    }
                }
                Part::Copy(buf) => {
                    out[..buf.len()].copy_from_slice(buf);
                }
            }
            Some(len)
        } else {
            None
        }
    }
}

/// 由一个或多个片段组成的完整格式化结果。
///
/// 结果既可以写入调用方提供的字节缓冲区，也可以在有分配能力的上层转换成字符串。
#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct Formatted<'a> {
    /// 表示符号的字节切片，只会是 `""`、`"-"` 或 `"+"`。
    pub sign: &'static str,
    /// 符号和可选补零之后需要渲染的格式化片段。
    pub parts: &'a [Part<'a>],
}

impl<'a> Formatted<'a> {
    /// 返回组合后的完整格式化结果的精确字节长度。
    pub fn len(&self) -> usize {
        self.sign.len() + self.parts.iter().map(|part| part.len()).sum::<usize>()
    }

    /// 把所有格式化片段写入提供的缓冲区。
    ///
    /// 成功时返回总写入字节数；缓冲区不足时返回 `None`。和 `Part::write` 一样，
    /// 失败时缓冲区内容可能已经被部分改写，调用方不能把这些字节当成有效输出。
    pub fn write(&self, out: &mut [u8]) -> Option<usize> {
        out.get_mut(..self.sign.len())?.copy_from_slice(self.sign.as_bytes());

        let mut written = self.sign.len();
        for part in self.parts {
            let len = part.write(&mut out[written..])?;
            written += len;
        }
        Some(written)
    }
}
