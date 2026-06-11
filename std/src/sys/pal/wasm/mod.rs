//! wasm/web 平台的系统绑定（System bindings）
//!
//! 本模块包含 wasm 上 OS 级功能的门面（facade，即平台特定）实现。
//! 注意这里的 wasm *不是* emscripten 或 wasi 的 wasm，因此这里没有运行时（runtime）。
//!
//! 这一切都还处于高度实验性阶段，目前并不真正打算用于广泛/生产环境，
//! 仍完全属于实验性范畴。随着时间推移，这些很可能会发生变化。
//!
//! 目前这里的所有函数基本上都是立即返回错误的桩（stub）。我们希望借助一个
//! 可移植性 lint，实际上可以直接移除所有这些代码：当我们为 wasm 编译时，
//! 干脆省略标准库中的相应部分。这样一来，对于那些注定会在运行时出错的东西，
//! 就能变成编译期错误！

#![deny(unsafe_op_in_unsafe_fn)]

#[path = "../unsupported/os.rs"]
pub mod os;
#[path = "../unsupported/time.rs"]
pub mod time;

#[cfg(target_feature = "atomics")]
#[path = "atomics/futex.rs"]
pub mod futex;

#[path = "../unsupported/common.rs"]
#[deny(unsafe_op_in_unsafe_fn)]
mod common;
pub use common::*;
