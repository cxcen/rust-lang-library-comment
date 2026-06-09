//! dec2flt 内部共用工具。

/// 处理不可变字节切片的辅助方法。
pub(crate) trait ByteSlice {
    /// 按小端序把 8 个字节读取为 64 位整数。
    fn read_u64(&self) -> u64;

    /// 按小端序把 64 位整数写成 8 个字节。
    fn write_u64(&mut self, value: u64);

    /// 计算两个切片之间的长度差。
    fn offset_from(&self, other: &Self) -> isize;

    /// 从字节切片中迭代解析并消费十进制数字。
    ///
    /// 返回去掉已消费数字后的剩余切片；遇到非数字字节时停止。
    fn parse_digits(&self, func: impl FnMut(u8)) -> &Self;
}

impl ByteSlice for [u8] {
    #[inline(always)] // 内联对消除边界检查很关键。
    fn read_u64(&self) -> u64 {
        let mut tmp = [0; 8];
        tmp.copy_from_slice(&self[..8]);
        u64::from_le_bytes(tmp)
    }

    #[inline(always)] // 内联对消除边界检查很关键。
    fn write_u64(&mut self, value: u64) {
        self[..8].copy_from_slice(&value.to_le_bytes())
    }

    #[inline]
    fn offset_from(&self, other: &Self) -> isize {
        other.len() as isize - self.len() as isize
    }

    #[inline]
    fn parse_digits(&self, mut func: impl FnMut(u8)) -> &Self {
        let mut s = self;

        while let Some((c, rest)) = s.split_first() {
            let c = c.wrapping_sub(b'0');
            if c < 10 {
                func(c);
                s = rest;
            } else {
                break;
            }
        }

        s
    }
}

/// 判断一个以 `u64` 表示的 8 字节字符串是否全由十进制数字组成。
///
/// 该检查不依赖字节被加载时的顺序；它只关心每个字节是否落在 ASCII `'0'..='9'`。
pub(crate) fn is_8digits(v: u64) -> bool {
    let a = v.wrapping_add(0x4646_4646_4646_4646);
    let b = v.wrapping_sub(0x3030_3030_3030_3030);
    (a | b) & 0x8080_8080_8080_8080 == 0
}

/// 自定义 64 位浮点中间表示，数值形态为 `m * 2^p`。
///
/// `p_biased` 已经带有目标格式的指数偏置，因此可以直接移入 IEEE 754 exponent 位。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct BiasedFp {
    /// 有效数字，也就是二进制 mantissa。
    pub m: u64,
    /// 带偏置的二进制 exponent。
    pub p_biased: i32,
}

impl BiasedFp {
    /// 表示 `0 * 2^p`。
    #[inline]
    pub const fn zero_pow2(p_biased: i32) -> Self {
        Self { m: 0, p_biased }
    }
}
