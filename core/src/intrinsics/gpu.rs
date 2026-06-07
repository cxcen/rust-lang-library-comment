//! 面向 GPU 目标平台的 intrinsic（编译器内建操作）。
//!
//! 本模块中的 intrinsic 专供 GPU 目标平台使用。它们可能与具体目标相关，
//! 但总体而言各 GPU 目标平台是相似的。
//!
//! 设计背景：intrinsic 是编译器内建操作，是 core 与编译器（rustc/LLVM 后端）之间的契约层；
//! 这类 GPU intrinsic 全部为 unstable（见下方 feature 门控），不会直接暴露给普通用户，
//! 而是由上层 GPU 相关 crate 封装后使用。

#![unstable(feature = "gpu_intrinsics", issue = "none")]

/// 返回指向 HSA 内核派发包（kernel dispatch packet）的指针。
///
/// 在 amdgpu 上，`gpu-kernel` 总是通过一个内核派发包来启动。派发包中包含工作组大小（workgroup size）、
/// 启动尺寸（launch size）以及其他数据。其内容由 [HSA 平台系统架构规范][HSA Platform System Architecture Specification]
/// 定义，例如 AMD 在 [hsa.h] 中的实现。
/// 该 intrinsic 返回一个 unit 指针（`*const ()`），这样 rustc 就无需知道派发包结构体的具体布局。
/// 该指针在整个程序生命周期内都有效。
///
/// [HSA Platform System Architecture Specification]: https://hsafoundation.com/wp-content/uploads/2021/02/HSA-SysArch-1.2.pdf
/// [hsa.h]: https://github.com/ROCm/rocm-systems/blob/rocm-7.1.0/projects/rocr-runtime/runtime/hsa-runtime/inc/hsa.h#L2959
#[rustc_nounwind]
#[rustc_intrinsic]
#[cfg(target_arch = "amdgpu")]
#[must_use = "returns a pointer that does nothing unless used"]
pub fn amdgpu_dispatch_ptr() -> *const ();
