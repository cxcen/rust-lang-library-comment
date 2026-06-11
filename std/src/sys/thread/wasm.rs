use crate::cmp;
use crate::time::Duration;

pub fn sleep(dur: Duration) {
    #[cfg(target_arch = "wasm32")]
    use core::arch::wasm32 as wasm;
    #[cfg(target_arch = "wasm64")]
    use core::arch::wasm64 as wasm;

    // 使用一次原子等待（atomic wait）配合给定的超时时间，人为地阻塞当前线程。
    // 注意，我们绝不应被通知（返回值为 0），我们的比较也绝不应失败（返回值为 1），
    // 因此我们应当总是只通过超时来恢复执行（返回值为 2）。
    let mut nanos = dur.as_nanos();
    while nanos > 0 {
        let amt = cmp::min(i64::MAX as u128, nanos);
        let mut x = 0;
        let val = unsafe { wasm::memory_atomic_wait32(&mut x, 0, amt as i64) };
        debug_assert_eq!(val, 2);
        nanos -= amt;
    }
}
