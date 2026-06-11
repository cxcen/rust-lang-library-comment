use crate::cell::Cell;
use crate::ops::{Deref, DerefMut};

/// 将一个值填充并对齐到一条缓存行（cache line）的长度。
///
/// 用途是消除“伪共享”（false sharing）：当两个本应彼此独立的字段恰好落在同一条缓存行上时，
/// 一个核心写其中一个字段会让另一核心持有的整条缓存行失效，造成无谓的缓存抖动。
/// 把热点字段各自塞进一个 `CachePadded` 即可保证它们分处不同缓存行。
#[derive(Clone, Copy, Default, Hash, PartialEq, Eq)]
// 从 Intel 的 Sandy Bridge 微架构开始，空间预取器（spatial prefetcher）一次会拉取成对的两条
// 64 字节缓存行，因此这里必须按 128 字节而非 64 字节对齐。
//
// 来源：
// - https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-optimization-manual.pdf
// - https://github.com/facebook/folly/blob/1b5288e6eea6df074758f877c849b6e73bbb9fbb/folly/lang/Align.h#L107
//
// ARM 的 big.LITTLE 架构核心是非对称的，其中“big”核心拥有 128 字节的缓存行。
//
// 来源：
// - https://www.mono-project.com/news/2016/09/12/arm64-icache/
//
// powerpc64 的缓存行大小为 128 字节。
//
// 来源：
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_ppc64x.go#L9
#[cfg_attr(
    any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "powerpc64",),
    repr(align(128))
)]
// arm、mips 和 mips64 的缓存行大小为 32 字节。
//
// 来源：
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_arm.go#L7
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_mips.go#L7
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_mipsle.go#L7
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_mips64x.go#L9
#[cfg_attr(
    any(
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
    ),
    repr(align(32))
)]
// s390x 的缓存行大小为 256 字节。
//
// 来源：
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_s390x.go#L7
#[cfg_attr(target_arch = "s390x", repr(align(256)))]
// x86、wasm 和 riscv 的缓存行大小为 64 字节。
//
// 来源：
// - https://github.com/golang/go/blob/dda2991c2ea0c5914714469c4defc2562a907230/src/internal/cpu/cpu_x86.go#L9
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_wasm.go#L7
// - https://github.com/golang/go/blob/5e31f78c8a4ed1b872ddc194f0cd1ae931b37d7e/src/internal/cpu/cpu_riscv64.go#L7
//
// 其余所有架构都假定缓存行大小为 64 字节。
#[cfg_attr(
    not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "s390x",
    )),
    repr(align(64))
)]
pub struct CachePadded<T> {
    value: T,
}

impl<T> CachePadded<T> {
    /// 将一个值填充并对齐到一条缓存行的长度。
    pub fn new(value: T) -> CachePadded<T> {
        CachePadded::<T> { value }
    }
}

impl<T> Deref for CachePadded<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for CachePadded<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

const SPIN_LIMIT: u32 = 6;

/// 在自旋循环（spin loop）中执行二次方退避（quadratic backoff）。
///
/// 随着重试次数增加，每次自旋的忙等次数按平方增长，从而在争用激烈时逐步减小对总线/缓存的压力。
pub struct Backoff {
    step: Cell<u32>,
}

impl Backoff {
    /// 创建一个新的 `Backoff`。
    pub fn new() -> Self {
        Backoff { step: Cell::new(0) }
    }

    /// 使用轻量级自旋进行退避。
    ///
    /// 当因为“其他线程取得了进展”而需要重试某个操作时（例如 CAS 失败），应使用此方法。
    #[inline]
    pub fn spin_light(&self) {
        let step = self.step.get().min(SPIN_LIMIT);
        for _ in 0..step.pow(2) {
            crate::hint::spin_loop();
        }

        self.step.set(self.step.get() + 1);
    }

    /// 使用重量级自旋进行退避。
    ///
    /// 当处于不允许把线程挂起（park）的阻塞循环中时，应使用此方法。
    /// 一旦自旋次数超过 `SPIN_LIMIT`，便改为让出（yield）当前线程。
    #[inline]
    pub fn spin_heavy(&self) {
        if self.step.get() <= SPIN_LIMIT {
            for _ in 0..self.step.get().pow(2) {
                crate::hint::spin_loop()
            }
        } else {
            crate::thread::yield_now();
        }

        self.step.set(self.step.get() + 1);
    }
}
