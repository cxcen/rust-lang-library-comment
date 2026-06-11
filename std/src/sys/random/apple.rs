//! Apple 平台上的随机数据。
//!
//! `CCRandomGenerateBytes` 会以 `kCCRandomDefault` 调用 `CCRandomCopyBytes`。
//! `CCRandomCopyBytes` 管理着一个 CSPRNG，它由内核的 CSPRNG 提供种子（seed）。
//! 我们使用 `CCRandomGenerateBytes` 而非 `SecCopyBytes`，因为前者可通过
//! `libSystem`（libc）访问，而后者则需要链接到 `Security.framework`。
//!
//! 注意，从技术上讲 `arc4random_buf` 同样可用，但它最终也是调用
//! 同一个系统服务，而 `CCRandomGenerateBytes` 已被证明
//! 兼容 App Store。

pub fn fill_bytes(bytes: &mut [u8]) {
    let ret = unsafe { libc::CCRandomGenerateBytes(bytes.as_mut_ptr().cast(), bytes.len()) };
    assert_eq!(ret, libc::kCCSuccess, "failed to generate random data");
}
