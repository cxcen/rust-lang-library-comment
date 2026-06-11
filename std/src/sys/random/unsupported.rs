use crate::ptr;

pub fn fill_bytes(_: &mut [u8]) {
    panic!("this target does not support random data generation");
}

pub fn hashmap_random_keys() -> (u64, u64) {
    // 用分配得到的地址来获取一点随机性。这并不
    // 特别安全，但也实在没有别的替代方案。
    let stack = 0u8;
    let heap = Box::new(0u8);
    let k1 = ptr::from_ref(&stack).addr() as u64;
    let k2 = ptr::from_ref(&*heap).addr() as u64;
    (k1, k2)
}
