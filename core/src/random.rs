//! 随机值生成。
//!
//! `core` 只定义“从随机源抽样”的抽象协议,不承诺随机源来自操作系统、硬件或密码学安全
//! 生成器。具体熵来源与安全属性由实现 [`RandomSource`] 的调用方负责说明。

use crate::range::RangeFull;

/// 随机字节来源。
///
/// 本 trait 表示“能填充随机字节”的来源,但不规定熵质量、可预测性或密码学安全性。
/// 若某个实现要用于密钥、nonce 等安全场景,实现者必须在自身文档中给出更强保证。
#[unstable(feature = "random", issue = "130703")]
pub trait RandomSource {
    /// 用随机字节填充 `bytes`。
    ///
    /// 注意,多次调用 `fill_bytes` 并不等价于用更大的缓冲区调用一次 `fill_bytes`。
    /// `RandomSource` 允许在这两种情况下返回不同字节序列。例如,实现可以一次生成一个机器字,
    /// 在不需要全部字节时丢弃其中一部分。
    fn fill_bytes(&mut self, bytes: &mut [u8]);
}

/// 表示某类型随机值分布的 trait。
#[unstable(feature = "random", issue = "130703")]
pub trait Distribution<T> {
    /// 使用指定随机源,从该分布中抽样一个随机值。
    fn sample(&self, source: &mut (impl RandomSource + ?Sized)) -> T;
}

impl<T, DT: Distribution<T>> Distribution<T> for &DT {
    fn sample(&self, source: &mut (impl RandomSource + ?Sized)) -> T {
        (*self).sample(source)
    }
}

impl Distribution<bool> for RangeFull {
    fn sample(&self, source: &mut (impl RandomSource + ?Sized)) -> bool {
        let byte: u8 = RangeFull.sample(source);
        byte & 1 == 1
    }
}

macro_rules! impl_primitive {
    ($t:ty) => {
        impl Distribution<$t> for RangeFull {
            fn sample(&self, source: &mut (impl RandomSource + ?Sized)) -> $t {
                let mut bytes = (0 as $t).to_ne_bytes();
                source.fill_bytes(&mut bytes);
                <$t>::from_ne_bytes(bytes)
            }
        }
    };
}

impl_primitive!(u8);
impl_primitive!(i8);
impl_primitive!(u16);
impl_primitive!(i16);
impl_primitive!(u32);
impl_primitive!(i32);
impl_primitive!(u64);
impl_primitive!(i64);
impl_primitive!(u128);
impl_primitive!(i128);
impl_primitive!(usize);
impl_primitive!(isize);
