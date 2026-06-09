//! 绑定到系统 `libm` 或 `libm` crate 提供的数学函数。
//!
//! 这些符号经由 `compiler-builtins` 暴露给 `core` 中的浮点方法。`core` 自身不含
//! 操作系统或标准库依赖，因此这里只声明 C ABI 入口，把平台具体实现留给目标系统的
//! `libm`，或者在不支持的平台上交给 `compiler-builtins` 提供的替代实现。

// SAFETY: 这些符号使用标准 C 接口；在有系统 `libm` 的平台由 `libm` 定义，
// 在不支持的平台由 `compiler-builtins` 提供。签名只包含按值传递和返回的
// 浮点数，不涉及 Rust 引用、别名或生命周期，因此 ABI 契约由这些外部符号维护。
unsafe extern "C" {
    pub(crate) safe fn cbrt(n: f64) -> f64;
    pub(crate) safe fn cbrtf(n: f32) -> f32;
    pub(crate) safe fn fdim(a: f64, b: f64) -> f64;
    pub(crate) safe fn fdimf(a: f32, b: f32) -> f32;
}
