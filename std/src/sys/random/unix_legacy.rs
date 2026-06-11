//! 来自 `/dev/urandom` 的随机数据
//!
//! 在 `getentropy` 于 2024 年被标准化之前，UNIX 并没有一种标准化的
//! 获取随机数据的方式，因此各系统只是沿袭了 Linux 开创的先例，
//! 在 `/dev/random` 和 `/dev/urandom` 处暴露随机设备。因此，
//! 对于那些既不支持 `arc4random_buf` 也不支持 `getentropy` 的少数系统，
//! 我们就直接从文件中读取。

use crate::fs::File;
use crate::io::Read;
use crate::sync::OnceLock;

static DEVICE: OnceLock<File> = OnceLock::new();

pub fn fill_bytes(bytes: &mut [u8]) {
    DEVICE
        .get_or_try_init(|| File::open("/dev/urandom"))
        .and_then(|mut dev| dev.read_exact(bytes))
        .expect("failed to generate random data");
}
