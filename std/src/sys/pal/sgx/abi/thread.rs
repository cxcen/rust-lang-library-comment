use fortanix_sgx_abi::Tcs;

/// 获取当前线程的 ID。该 ID 保证在 enclave 中所有当前运行的线程之间唯一，并且
/// 保证在该线程的生命周期内保持不变。更具体地说，对于 SGX，该 ID 与 TCS 的地址
/// 之间存在一一对应关系。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn current() -> Tcs {
    unsafe extern "C" {
        fn get_tcs_addr() -> *mut u8;
    }
    let addr = unsafe { get_tcs_addr() };
    match Tcs::new(addr) {
        Some(tcs) => tcs,
        None => rtabort!("TCS must not be placed at address zero (this is a linker error)"),
    }
}
