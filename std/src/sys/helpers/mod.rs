//! 在 `sys` 内部使用的小型辅助函数。
//!
//! 如果其中任何函数在 `sys` 之外也有用武之地，请把它移到
//! 其他模块中。

#[cfg_attr(not(target_os = "linux"), allow(unused))] // 并非所有平台都会用到。
mod small_c_string;
#[cfg_attr(not(target_os = "windows"), allow(unused))] // 并非所有平台都会用到。
mod wstr;

#[cfg(test)]
mod tests;

#[cfg_attr(not(target_os = "linux"), allow(unused))] // 并非所有平台都会用到。
pub use small_c_string::{run_path_with_cstr, run_with_cstr};
#[cfg_attr(not(target_os = "windows"), allow(unused))] // 并非所有平台都会用到。
pub use wstr::WStrUnits;

/// 在不溢出的前提下计算 `(value*numerator)/denom`，只要
/// `numerator*denom` 与最终结果都能放进 `u64`（对于我们的时间换算而言确实如此）。
#[cfg_attr(not(target_os = "windows"), allow(unused))] // 并非所有平台都会用到。
pub fn mul_div_u64(value: u64, numerator: u64, denom: u64) -> u64 {
    let q = value / denom;
    let r = value % denom;
    // 把 value 分解为 (value/denom*denom + value%denom)，
    // 代入 (value*numerator)/denom 并化简。
    // 由于 r < denom，所以 (denom*numerator) 是 (r*numerator) 的上界
    q * numerator + r * numerator / denom
}

#[cfg_attr(not(target_os = "linux"), allow(unused))] // 并非所有平台都会用到。
pub fn ignore_notfound<T>(result: crate::io::Result<T>) -> crate::io::Result<()> {
    match result {
        Err(err) if err.kind() == crate::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Ok(()),
        Err(err) => Err(err),
    }
}
