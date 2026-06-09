use crate::mem::MaybeUninit;

/// 描述某个整数类型在十进制表示中最多需要多少位数字的 trait。
///
/// `NumBuffer` 依赖这个上界来为无分配的整数格式化准备足够大的栈上缓冲区。
/// 这个 trait 的实现必须与目标整数类型的最大十进制长度一致,否则写入十进制
/// 文本时可能越过缓冲区或留下未初始化字节。
#[unstable(feature = "int_format_into", issue = "138215")]
pub trait NumBufferTrait {
    /// 该整数类型的十进制表示最多包含的数字个数。
    const BUF_SIZE: usize;
}

macro_rules! impl_NumBufferTrait {
    ($($signed:ident, $unsigned:ident,)*) => {
        $(
            #[unstable(feature = "int_format_into", issue = "138215")]
            impl NumBufferTrait for $signed {
                // 使用 `+ 2` 而不是 `+ 1`,是为了把负号 `-` 也计入缓冲区容量。
                const BUF_SIZE: usize = $signed::MAX.ilog(10) as usize + 2;
            }
            #[unstable(feature = "int_format_into", issue = "138215")]
            impl NumBufferTrait for $unsigned {
                const BUF_SIZE: usize = $unsigned::MAX.ilog(10) as usize + 1;
            }
        )*
    }
}

impl_NumBufferTrait! {
    i8, u8,
    i16, u16,
    i32, u32,
    i64, u64,
    isize, usize,
    i128, u128,
}

/// 根据关联整数类型的最大十进制位数确定内部容量的缓冲区包装器。
///
/// 该类型服务于 `fmt` 的热路径:调用者提供可变缓冲区,整数格式化代码把数字写入
/// 其中,再把已初始化的后缀作为 `str` 返回。缓冲区字节使用 `MaybeUninit`,
/// 因为格式化会从末尾向前填充,未使用的前缀不应被读取。
#[unstable(feature = "int_format_into", issue = "138215")]
#[derive(Debug)]
pub struct NumBuffer<T: NumBufferTrait> {
    // FIXME: 一旦 const generics 支持这里需要的形式,就用 `T::BUF_SIZE` 取代 40。
    pub(crate) buf: [MaybeUninit<u8>; 40],
    // FIXME: 当数组长度能真正使用 `T` 后,移除这个字段。
    phantom: core::marker::PhantomData<T>,
}

#[unstable(feature = "int_format_into", issue = "138215")]
impl<T: NumBufferTrait> NumBuffer<T> {
    /// 创建一个尚未填入数字的内部缓冲区。
    #[unstable(feature = "int_format_into", issue = "138215")]
    pub const fn new() -> Self {
        // FIXME: 一旦 const generics 支持这里需要的形式,就用 `T::BUF_SIZE` 取代 40。
        NumBuffer { buf: [MaybeUninit::<u8>::uninit(); 40], phantom: core::marker::PhantomData }
    }

    /// 返回内部缓冲区的总长度。
    #[unstable(feature = "int_format_into", issue = "138215")]
    pub const fn capacity(&self) -> usize {
        self.buf.len()
    }
}
