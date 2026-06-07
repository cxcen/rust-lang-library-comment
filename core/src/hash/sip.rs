//! SipHash 的实现。

#![allow(deprecated)] // 本模块中的类型已经弃用

use crate::marker::PhantomData;
use crate::{cmp, ptr};

/// SipHash 1-3 的实现。
///
/// 这是标准库当前使用的默认哈希函数之一，例如 `collections::HashMap` 默认会通过
/// `DefaultHasher` 使用它。`SipHasher13` 使用较少的压缩轮数，在性能和抗 Hash-DoS
/// 能力之间做了标准库内部使用所需的折中。
///
/// 参见：<https://github.com/veorq/SipHash>
#[unstable(feature = "hashmap_internals", issue = "none")]
#[deprecated(since = "1.13.0", note = "use `std::hash::DefaultHasher` instead")]
#[derive(Debug, Clone, Default)]
#[doc(hidden)]
pub struct SipHasher13 {
    hasher: Hasher<Sip13Rounds>,
}

/// SipHash 2-4 的实现。
///
/// 参见：<https://github.com/veorq/SipHash>
#[unstable(feature = "hashmap_internals", issue = "none")]
#[deprecated(since = "1.13.0", note = "use `std::hash::DefaultHasher` instead")]
#[derive(Debug, Clone, Default)]
struct SipHasher24 {
    hasher: Hasher<Sip24Rounds>,
}

/// SipHash 2-4 的实现。
///
/// 参见：<https://github.com/veorq/SipHash>
///
/// SipHash 是通用哈希函数：它有不错的运行速度，可以和 Spooky、City 这类哈希算法竞争，
/// 同时支持强 _keyed_ hashing。keyed hashing 意味着同一输入在不同密钥下会得到不同输出，
/// 因而哈希表可以从强随机数生成器，例如
/// [`rand::os::OsRng`](https://docs.rs/rand/latest/rand/rngs/struct.OsRng.html)，
/// 取得密钥，降低攻击者提前构造大量碰撞键的可行性。
///
/// 虽然 SipHash 通常被认为强度较好，但这个实现面向哈希表防碰撞和通用 keyed hashing，
/// 并不以密码学用途为目标。因此，_强烈不建议_ 把此实现用于任何密码学场景。
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "1.13.0", note = "use `std::hash::DefaultHasher` instead")]
#[derive(Debug, Clone, Default)]
pub struct SipHasher(SipHasher24);

#[derive(Debug)]
struct Hasher<S: Sip> {
    k0: u64,
    k1: u64,
    length: usize, // 已处理的字节数
    state: State,  // hash 的 `State`
    tail: u64,     // 尚未处理的尾部字节，按小端(little-endian)组装
    ntail: usize,  // `tail` 中有效字节的数量
    _marker: PhantomData<S>,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct State {
    // 在 SipHash 算法中，v0/v2 与 v1/v3 会成对出现；SipHash 的 simd 实现也会使用
    // v02 和 v13 这样的向量。把字段按这个顺序放在结构体里，编译器可以自行识别出少量
    // simd 优化机会。
    v0: u64,
    v2: u64,
    v1: u64,
    v3: u64,
}

macro_rules! compress {
    ($state:expr) => {{ compress!($state.v0, $state.v1, $state.v2, $state.v3) }};
    ($v0:expr, $v1:expr, $v2:expr, $v3:expr) => {{
        $v0 = $v0.wrapping_add($v1);
        $v2 = $v2.wrapping_add($v3);
        $v1 = $v1.rotate_left(13);
        $v1 ^= $v0;
        $v3 = $v3.rotate_left(16);
        $v3 ^= $v2;
        $v0 = $v0.rotate_left(32);

        $v2 = $v2.wrapping_add($v1);
        $v0 = $v0.wrapping_add($v3);
        $v1 = $v1.rotate_left(17);
        $v1 ^= $v2;
        $v3 = $v3.rotate_left(21);
        $v3 ^= $v0;
        $v2 = $v2.rotate_left(32);
    }};
}

/// 从字节流中按小端(little-endian)顺序加载指定整数类型。这里使用 `copy_nonoverlapping`，
/// 让编译器为可能未对齐的地址生成高效加载代码，而不是把未对齐指针直接转成整数指针。
///
/// # 安全性(Safety）
///
/// 此宏会对 `$buf` 的 `$i..$i+size_of::<$int_ty>()` 范围执行未检查索引，因此调用方必须
/// 保证该范围完全位于 `$buf` 之内。目标整数是局部变量，写入目标的字节数正好等于该类型
/// 大小，因此额外要求集中在源切片边界，而不是对齐。
macro_rules! load_int_le {
    ($buf:expr, $i:expr, $int_ty:ident) => {{
        debug_assert!($i + size_of::<$int_ty>() <= $buf.len());
        let mut data = 0 as $int_ty;
        ptr::copy_nonoverlapping(
            $buf.as_ptr().add($i),
            &mut data as *mut _ as *mut u8,
            size_of::<$int_ty>(),
        );
        data.to_le()
    }};
}

/// 使用字节切片中最多 7 个字节加载一个 `u64`。写法看起来笨重，但经由 `load_int_le!`
/// 产生的 `copy_nonoverlapping` 调用大小都是固定的，可以避免调用 `memcpy`，对性能更有利。
///
/// # 安全性(Safety）
///
/// 此函数会对 `buf` 的 `start..start+len` 范围执行未检查索引，因此调用方必须保证该范围
/// 完全位于 `buf` 之内。`len` 还必须小于 8；函数体用 `debug_assert!` 检查这一点，但
/// unsafe 前置条件不能只依赖调试断言。
#[inline]
unsafe fn u8to64_le(buf: &[u8], start: usize, len: usize) -> u64 {
    debug_assert!(len < 8);
    let mut i = 0; // 输出 `u64` 中当前字节索引，从低位字节开始
    let mut out = 0;
    if i + 3 < len {
        // SAFETY: `i` 不可能大于 `len`，并且调用方必须保证索引范围
        // `start..start+len` 在边界内。
        out = unsafe { load_int_le!(buf, start + i, u32) } as u64;
        i += 4;
    }
    if i + 1 < len {
        // SAFETY: 理由同上。
        out |= (unsafe { load_int_le!(buf, start + i, u16) } as u64) << (i * 8);
        i += 2
    }
    if i < len {
        // SAFETY: 理由同上。
        out |= (unsafe { *buf.get_unchecked(start + i) } as u64) << (i * 8);
        i += 1;
    }
    //FIXME(fee1-dead): 使用 debug_assert_eq
    debug_assert!(i == len);
    out
}

impl SipHasher {
    /// 创建新的 `SipHasher`，两个初始密钥都设为 0。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[deprecated(since = "1.13.0", note = "use `std::hash::DefaultHasher` instead")]
    #[must_use]
    pub fn new() -> SipHasher {
        SipHasher::new_with_keys(0, 0)
    }

    /// 创建使用给定密钥的 `SipHasher`。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[deprecated(since = "1.13.0", note = "use `std::hash::DefaultHasher` instead")]
    #[must_use]
    pub fn new_with_keys(key0: u64, key1: u64) -> SipHasher {
        SipHasher(SipHasher24 { hasher: Hasher::new_with_keys(key0, key1) })
    }
}

impl SipHasher13 {
    /// 创建新的 `SipHasher13`，两个初始密钥都设为 0。
    #[inline]
    #[unstable(feature = "hashmap_internals", issue = "none")]
    #[rustc_const_unstable(feature = "const_default", issue = "143894")]
    #[deprecated(since = "1.13.0", note = "use `std::hash::DefaultHasher` instead")]
    pub const fn new() -> SipHasher13 {
        SipHasher13::new_with_keys(0, 0)
    }

    /// 创建使用给定密钥的 `SipHasher13`。
    #[inline]
    #[unstable(feature = "hashmap_internals", issue = "none")]
    #[rustc_const_unstable(feature = "const_default", issue = "143894")]
    #[deprecated(since = "1.13.0", note = "use `std::hash::DefaultHasher` instead")]
    pub const fn new_with_keys(key0: u64, key1: u64) -> SipHasher13 {
        SipHasher13 { hasher: Hasher::new_with_keys(key0, key1) }
    }
}

impl<S: Sip> Hasher<S> {
    #[inline]
    const fn new_with_keys(key0: u64, key1: u64) -> Hasher<S> {
        let mut state = Hasher {
            k0: key0,
            k1: key1,
            length: 0,
            state: State { v0: 0, v1: 0, v2: 0, v3: 0 },
            tail: 0,
            ntail: 0,
            _marker: PhantomData,
        };
        state.reset();
        state
    }

    #[inline]
    const fn reset(&mut self) {
        self.length = 0;
        self.state.v0 = self.k0 ^ 0x736f6d6570736575;
        self.state.v1 = self.k1 ^ 0x646f72616e646f6d;
        self.state.v2 = self.k0 ^ 0x6c7967656e657261;
        self.state.v3 = self.k1 ^ 0x7465646279746573;
        self.ntail = 0;
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl super::Hasher for SipHasher {
    #[inline]
    fn write(&mut self, msg: &[u8]) {
        self.0.hasher.write(msg)
    }

    #[inline]
    fn write_str(&mut self, s: &str) {
        self.0.hasher.write_str(s);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0.hasher.finish()
    }
}

#[unstable(feature = "hashmap_internals", issue = "none")]
impl super::Hasher for SipHasher13 {
    #[inline]
    fn write(&mut self, msg: &[u8]) {
        self.hasher.write(msg)
    }

    #[inline]
    fn write_str(&mut self, s: &str) {
        self.hasher.write_str(s);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hasher.finish()
    }
}

impl<S: Sip> super::Hasher for Hasher<S> {
    // 注意：此类型没有定义整数专用哈希方法(`write_u*`、`write_i*`)。可以添加这些方法，
    // 复制 librustc_data_structures/sip128.rs 中的 `short_write` 实现，并为
    // `SipHasher`、`SipHasher13` 和 `DefaultHasher` 添加 `write_u*`/`write_i*` 方法。
    // 这样会显著加快这些 hasher 的整数哈希速度，但代价是在某些基准测试中略微拖慢编译速度。
    // 细节见 #69152。
    #[inline]
    fn write(&mut self, msg: &[u8]) {
        let length = msg.len();
        self.length += length;

        let mut needed = 0;

        if self.ntail != 0 {
            needed = 8 - self.ntail;
            // SAFETY: `cmp::min(length, needed)` 保证不会超过 `length`。
            self.tail |= unsafe { u8to64_le(msg, 0, cmp::min(length, needed)) } << (8 * self.ntail);
            if length < needed {
                self.ntail += length;
                return;
            } else {
                self.state.v3 ^= self.tail;
                S::c_rounds(&mut self.state);
                self.state.v0 ^= self.tail;
                self.ntail = 0;
            }
        }

        // 已缓冲的尾部现在已经刷新，开始处理新的输入。
        let len = length - needed;
        let left = len & 0x7; // len % 8

        let mut i = needed;
        while i < len - left {
            // SAFETY: `len - left` 是不超过 `len` 的最大 8 的倍数；`i` 从 `needed` 开始，
            // 且 `len` 等于 `length - needed`，因此 `i + 8` 保证小于等于 `length`。
            let mi = unsafe { load_int_le!(msg, i, u64) };

            self.state.v3 ^= mi;
            S::c_rounds(&mut self.state);
            self.state.v0 ^= mi;

            i += 8;
        }

        // SAFETY: 此时 `i` 等于 `needed + len.div_euclid(8) * 8`，所以
        // `i + left` = `needed + len` = `length`，而 `length` 按定义等于 `msg.len()`。
        self.tail = unsafe { u8to64_le(msg, i, left) };
        self.ntail = left;
    }

    #[inline]
    fn write_str(&mut self, s: &str) {
        // 此 hasher 按字节工作，而 `str` 中不会出现 `0xFF`，因此只额外写入这个字节就足以
        // 维持 prefix-free。
        self.write(s.as_bytes());
        self.write_u8(0xFF);
    }

    #[inline]
    fn finish(&self) -> u64 {
        let mut state = self.state;

        let b: u64 = ((self.length as u64 & 0xff) << 56) | self.tail;

        state.v3 ^= b;
        S::c_rounds(&mut state);
        state.v0 ^= b;

        state.v2 ^= 0xff;
        S::d_rounds(&mut state);

        state.v0 ^ state.v1 ^ state.v2 ^ state.v3
    }
}

impl<S: Sip> Clone for Hasher<S> {
    #[inline]
    fn clone(&self) -> Hasher<S> {
        Hasher {
            k0: self.k0,
            k1: self.k1,
            length: self.length,
            state: self.state,
            tail: self.tail,
            ntail: self.ntail,
            _marker: self._marker,
        }
    }
}

#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<S: Sip> const Default for Hasher<S> {
    /// 创建 `Hasher<S>`，两个初始密钥都设为 0。
    #[inline]
    fn default() -> Hasher<S> {
        Hasher::new_with_keys(0, 0)
    }
}

#[doc(hidden)]
trait Sip {
    fn c_rounds(_: &mut State);
    fn d_rounds(_: &mut State);
}

#[derive(Debug, Clone, Default)]
struct Sip13Rounds;

impl Sip for Sip13Rounds {
    #[inline]
    fn c_rounds(state: &mut State) {
        compress!(state);
    }

    #[inline]
    fn d_rounds(state: &mut State) {
        compress!(state);
        compress!(state);
        compress!(state);
    }
}

#[derive(Debug, Clone, Default)]
struct Sip24Rounds;

impl Sip for Sip24Rounds {
    #[inline]
    fn c_rounds(state: &mut State) {
        compress!(state);
        compress!(state);
    }

    #[inline]
    fn d_rounds(state: &mut State) {
        compress!(state);
        compress!(state);
        compress!(state);
        compress!(state);
    }
}
