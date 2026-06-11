use rand::{RngCore, SeedableRng};

use crate::hash::{BuildHasher, Hash, Hasher, RandomState};
use crate::panic::Location;
use crate::path::{Path, PathBuf};
use crate::{env, fs, thread};

/// 仅用于测试，替代 `rand::thread_rng()`——后者对我们不可用，因为我们希望
/// 允许在 tier-3 目标上运行标准库测试，而这些目标可能没有 `getrandom` 支持。
///
/// 这里做了一点花哨处理，确保每次调用得到的种子都不同（某些测试很遗憾地依赖
/// 这一点），但也没刻意做得很彻底。
///
/// 此函数在 `core`、`alloc` 测试套件（以及 `std` 的集成测试）里都有重复；但要
/// 设计一套共享机制似乎远比把这 7 行函数复制几遍更麻烦——毕竟即便用一个永久
/// 不稳定的 feature，我们也不希望从 `std` 暴露 `rand` 里的类型。
#[track_caller]
pub(crate) fn test_rng() -> rand_xorshift::XorShiftRng {
    let mut hasher = RandomState::new().build_hasher();
    Location::caller().hash(&mut hasher);
    let hc64 = hasher.finish();
    let seed_vec = hc64.to_le_bytes().into_iter().chain(0u8..8).collect::<Vec<u8>>();
    let seed: [u8; 16] = seed_vec.as_slice().try_into().unwrap();
    SeedableRng::from_seed(seed)
}

pub struct TempDir(PathBuf);

impl TempDir {
    pub fn join(&self, path: &str) -> PathBuf {
        let TempDir(ref p) = *self;
        p.join(path)
    }

    pub fn path(&self) -> &Path {
        let TempDir(ref p) = *self;
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // 哎，既然我们正在测试 fs 模块，那可真希望这个删除操作本身实现得没问题！
        let TempDir(ref p) = *self;
        let result = fs::remove_dir_all(p);
        // 避免在 panic 过程中再次 panic，因为那会让进程立即 abort，
        // 不显示测试结果。
        if !thread::panicking() {
            result.unwrap();
        }
    }
}

#[track_caller] // for `test_rng`
pub fn tmpdir() -> TempDir {
    let p = env::temp_dir();
    let mut r = test_rng();
    let ret = p.join(&format!("rust-{}", r.next_u32()));
    fs::create_dir(&ret).unwrap();
    TempDir(ret)
}
