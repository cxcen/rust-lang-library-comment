//! 使用 Zircon 内核生成随机数据。
//!
//! Fuchsia 一如既往地相当友好，恰好提供了我们所需要的 API：
//! <https://fuchsia.dev/reference/syscalls/cprng_draw>。

#[link(name = "zircon")]
unsafe extern "C" {
    fn zx_cprng_draw(buffer: *mut u8, len: usize);
}

pub fn fill_bytes(bytes: &mut [u8]) {
    unsafe { zx_cprng_draw(bytes.as_mut_ptr(), bytes.len()) }
}
