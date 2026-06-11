//! 通过 `getentropy` 生成随机数据。
//!
//! 自第 8 号议题（issue 8，2024 年）起，POSIX 规范强制要求存在
//! `getentropy` 函数，它会用随机数据填充一个最多 `GETENTROPY_MAX` 字节
//!（在所有已知平台上为 256）的切片。遗憾的是，它的本意只是用来为其他 CPRNG
//! 提供种子，而我们并没有这样的 CPRNG，因此我们只在 `arc4random_buf` 之类
//! 不可用或不安全的平台上使用它（目前仅 Emscripten 属于此种情况）。

pub fn fill_bytes(bytes: &mut [u8]) {
    // GETENTROPY_MAX 在大多数平台上尚未定义，但它被强制要求
    // 至少为 256，因此我们就用 256 作为上限。
    for chunk in bytes.chunks_mut(256) {
        let r = unsafe { libc::getentropy(chunk.as_mut_ptr().cast(), chunk.len()) };
        assert_ne!(r, -1, "failed to generate random data");
    }
}
