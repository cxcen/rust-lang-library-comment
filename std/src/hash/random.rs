//! 本模块的存在是为了把 [`RandomState`] 与 [`DefaultHasher`] 从 [`collections`]
//! 模块中隔离出来，同时又不真正对外公开导出它们，这样该实现的某些部分将来
//! 可以更容易地迁移到 [`alloc`] crate。
//!
//! 尽管这里的项是 public 的并带有稳定性属性（stability attributes），它们实际上
//! 无法在本 crate 之外被访问。
//!
//! [`collections`]: crate::collections

#[allow(deprecated)]
use super::{BuildHasher, Hasher, SipHasher13};
use crate::cell::Cell;
use crate::fmt;
use crate::sys::random::hashmap_random_keys;

/// `RandomState` 是 [`HashMap`] 类型默认使用的状态（state）。
///
/// 同一个 `RandomState` 实例会创建出相同的 [`Hasher`] 实例，但两个不同的
/// `RandomState` 实例所创建的 hasher，对于相同的值不太可能产生相同的结果。
///
/// [`HashMap`]: crate::collections::HashMap
///
/// # 示例
///
/// ```
/// use std::collections::HashMap;
/// use std::hash::RandomState;
///
/// let s = RandomState::new();
/// let mut map = HashMap::with_hasher(s);
/// map.insert(1, 2);
/// ```
#[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
#[derive(Clone)]
pub struct RandomState {
    k0: u64,
    k1: u64,
}

impl RandomState {
    /// 构造一个用随机 key 初始化的新 `RandomState`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::hash::RandomState;
    ///
    /// let s = RandomState::new();
    /// ```
    #[inline]
    #[allow(deprecated)]
    // rand
    #[must_use]
    #[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
    pub fn new() -> RandomState {
        // 历史上，本函数并不缓存来自操作系统（OS）的 key，而是每次都简单地
        // 调用两次 `rand::thread_rng().gen()`。但在 #31356 中发现：由于我们会
        // 周期性地用 OS 的随机源对线程本地（thread-local）RNG 重新播种
        // （re-seed），当在一个线程上创建大量 hash map 时，这会造成过度的性能
        // 下降。为解决这个性能陷阱，我们按线程缓存第一组随机生成的 key。
        //
        // 后来在 #36481 中又发现：暴露一个确定性的迭代顺序会被用于发起一种
        // 形式的 DOS 攻击。为对抗这一点，我们在每次创建 RandomState 时都将其中
        // 一个种子（seed）递增，从而让每个对应的 HashMap 都拥有不同的迭代顺序。
        thread_local!(static KEYS: Cell<(u64, u64)> = {
            Cell::new(hashmap_random_keys())
        });

        KEYS.with(|keys| {
            let (k0, k1) = keys.get();
            keys.set((k0.wrapping_add(1), k1));
            RandomState { k0, k1 }
        })
    }
}

#[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
impl BuildHasher for RandomState {
    type Hasher = DefaultHasher;
    #[inline]
    #[allow(deprecated)]
    fn build_hasher(&self) -> DefaultHasher {
        DefaultHasher(SipHasher13::new_with_keys(self.k0, self.k1))
    }
}

/// [`RandomState`] 所使用的默认 [`Hasher`]。
///
/// 其内部算法未作规定（not specified），因此不应跨多个发行版（releases）依赖
/// 该算法本身及其产生的 hash 值。
#[allow(deprecated)]
#[derive(Clone, Debug)]
#[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
pub struct DefaultHasher(SipHasher13);

impl DefaultHasher {
    /// 创建一个新的 `DefaultHasher`。
    ///
    /// 本 hasher 并不保证与其他所有 `DefaultHasher` 实例相同，但它与所有其他
    /// 通过 `new` 或 `default` 创建的 `DefaultHasher` 实例相同。
    #[stable(feature = "hashmap_default_hasher", since = "1.13.0")]
    #[inline]
    #[allow(deprecated)]
    #[rustc_const_unstable(feature = "const_default", issue = "143894")]
    #[must_use]
    pub const fn new() -> DefaultHasher {
        DefaultHasher(SipHasher13::new_with_keys(0, 0))
    }
}

#[stable(feature = "hashmap_default_hasher", since = "1.13.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl const Default for DefaultHasher {
    /// 使用 [`new`] 创建一个新的 `DefaultHasher`。
    /// 更多信息参见其文档。
    ///
    /// [`new`]: DefaultHasher::new
    #[inline]
    fn default() -> DefaultHasher {
        DefaultHasher::new()
    }
}

#[stable(feature = "hashmap_default_hasher", since = "1.13.0")]
impl Hasher for DefaultHasher {
    // 底层的 `SipHasher13` 并未覆写（override）其他的 `write_*` 方法，
    // 因此这里不转发它们也是没问题的。

    #[inline]
    fn write(&mut self, msg: &[u8]) {
        self.0.write(msg)
    }

    #[inline]
    fn write_str(&mut self, s: &str) {
        self.0.write_str(s);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0.finish()
    }
}

#[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
impl Default for RandomState {
    /// 构造一个新的 `RandomState`。
    #[inline]
    fn default() -> RandomState {
        RandomState::new()
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for RandomState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RandomState").finish_non_exhaustive()
    }
}
