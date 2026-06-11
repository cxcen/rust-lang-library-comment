//! 本模块包含组件模型（component model）中 `cabi_realloc` 函数的规范定义。
//!
//! 组件模型用于在内存中表示数据类型的规范 ABI（canonical ABI），
//! 在传递列表（list）和字符串（string）等数据时会用到此函数。
//! 此函数的行为类似于 C 的 `realloc`，但同时还会考虑对齐（alignment）。
//!
//! 值得注意的是，组件并非必须导出此函数，但目前几乎所有组件最终都会这样做。
//! 在标准库中提供这一定义，免除了每次编译都要自行定义它的需要。
//!
//! 关于规范 ABI 的更多信息可参见
//! <https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md>
//!
//! 注意，此函数的名称目前在规范 ABI 中尚未标准化。相反，它是“组件化过程”
//! （componentization process，即把一个核心 wasm 模块转换为组件）的一种约定，
//! 该过程会采用这个名称。此外，这并不是此函数唯一可能的定义，因此
//! 它被定义为一个“弱（weak）”符号。这意味着如果某次编译中存在其他定义，
//! 则允许它们覆盖此定义。

use crate::alloc::{self, Layout};
use crate::ptr;

#[used]
static FORCE_CODEGEN_OF_CABI_REALLOC: unsafe extern "C" fn(
    *mut u8,
    usize,
    usize,
    usize,
) -> *mut u8 = cabi_realloc;

#[linkage = "weak"]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cabi_realloc(
    old_ptr: *mut u8,
    old_len: usize,
    align: usize,
    new_len: usize,
) -> *mut u8 {
    let layout;
    let ptr = if old_len == 0 {
        if new_len == 0 {
            return ptr::without_provenance_mut(align);
        }
        layout = Layout::from_size_align_unchecked(new_len, align);
        alloc::alloc(layout)
    } else {
        debug_assert_ne!(new_len, 0, "non-zero old_len requires non-zero new_len!");
        layout = Layout::from_size_align_unchecked(old_len, align);
        alloc::realloc(old_ptr, layout, new_len)
    };
    if ptr.is_null() {
        // 在 debug 模式下打印一条友好的提示信息，但在 release 模式下
        // 不引入那么多与打印相关的依赖，因此只发出一条 `unreachable` 指令。
        if cfg!(debug_assertions) {
            alloc::handle_alloc_error(layout);
        } else {
            super::abort_internal();
        }
    }
    return ptr;
}
