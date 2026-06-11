//! 在 Emscripten 上，Rust panic 被包裹进 C++ 异常中，所以我们只是转发到
//! 由 Emscripten 提供的 `__gxx_personality_v0`。

use unwind as uw;

use crate::ffi::c_int;

// 编译器要求它存在（例如它是一个 lang item），但编译器实际上从不会调用它。
// Emscripten 的 EH（异常处理）根本不使用 personality 函数，它转而使用
// __cxa_find_matching_catch。Wasm 错误处理则会使用 __gxx_personality_wasm0。
#[lang = "eh_personality"]
unsafe extern "C" fn rust_eh_personality(
    _version: c_int,
    _actions: uw::_Unwind_Action,
    _exception_class: uw::_Unwind_Exception_Class,
    _exception_object: *mut uw::_Unwind_Exception,
    _context: *mut uw::_Unwind_Context,
) -> uw::_Unwind_Reason_Code {
    core::intrinsics::abort()
}
