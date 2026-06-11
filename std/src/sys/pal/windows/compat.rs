//! 用于支持旧版本 Windows 的“兼容层（compatibility layer）”
//!
//! 标准库使用了一些在旧版本 Windows 上并不存在的 Windows API 函数。
//! （注意：Rust 支持的最旧 Windows 版本是 Windows 7（客户端）和
//! Windows Server 2008（服务器）。）本模块实现了一种延迟 DLL 导入绑定的形式，
//! 使用 `GetModuleHandle` 和 `GetProcAddress` 在运行时查找 DLL 的入口点。
//!
//! 其实现方式很简单：把一个函数指针存放在一个原子变量中。与调用任何其他
//! 动态导入的函数相比，加载并调用这个函数几乎没有额外开销。
//!
//! 所存储的函数指针起初是一个导入器（importer）函数，它会在首次被调用时
//! 把自己替换为真正的函数。如果真正的函数无法被导入，则用一个后备（fallback）
//! 函数取而代之。虽然这对于顺利路径（即函数已经加载的情况）开销很低，
//! 但确实意味着函数首次被调用时会有一些开销。最坏情况下，多个线程可能会
//! 全都不必要地导入同一个函数。

use crate::ffi::{CStr, c_void};
use crate::ptr::NonNull;
use crate::sys::c;

// 这里使用一个静态初始化器来预加载一些导入的函数。
// CRT（C 运行时）会在 `main` 被调用之前（对于可执行文件）以及在 `DllMain`
// 被调用之前（对于 DLL）执行静态初始化器。
//
// 其工作原理是向 `.CRT$XCT` 节贡献一个全局符号。链接器会构建一张包含所有
// 静态初始化器函数的表。随后 CRT 启动代码会遍历该表，逐个调用每个初始化器函数。
//
// NOTE: 用户代码应改用 .CRT$XCU，以可靠地在 std 的初始化器之后运行。
// 如果你正在阅读这段注释并希望在这里得到某种保证，请提交一个 issue 进行讨论；
// 目前我们不对 main 之前的任何功能做出保证。
// 参见 https://docs.microsoft.com/en-us/cpp/c-runtime-library/crt-initialization?view=msvc-170
#[cfg(target_vendor = "win7")]
#[used]
#[unsafe(link_section = ".CRT$XCT")]
static INIT_TABLE_ENTRY: unsafe extern "C" fn() = init;

/// 预加载一些导入的函数。
///
/// 注意：这里包含的任何函数都会被无条件地加载进最终的二进制文件中，
/// 无论它们是否真的会被用到。
///
/// 因此，这里应当仅限于那些必须预加载的 `compat_fn_optional` 函数，
/// 或者那些在实际场景中更惰性的加载方式会带来负面性能影响的函数。
///
/// 目前我们只预加载 `WaitOnAddress` 和 `WakeByAddressSingle`。
#[cfg(target_vendor = "win7")]
unsafe extern "C" fn init() {
    // 在 exe 中，这段代码在 main() 之前执行，因此是单线程的。
    // 在 DLL 中，系统加载器锁（loader lock）会被持有，从而对访问进行同步。
    // 所以这里适用的最佳实践与在 DllMain 中运行时是一样的：
    // https://docs.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-best-practices
    //
    // 不要在本函数中做任何有趣或复杂的事情！不要调用任何会触及全局状态的
    // Rust 函数或 CRT 函数，因为本函数是在全局初始化期间运行的。例如，
    // 不要做任何动态分配、不要调用 LoadLibrary，等等。

    // 尝试预加载这些同步（synch）函数。
    load_synch_functions();
}

/// 用于从字面量和符号名创建 CStr 的辅助宏。
macro_rules! ansi_str {
    (sym $ident:ident) => {{ crate::sys::compat::const_cstr_from_bytes(concat!(stringify!($ident), "\0").as_bytes()) }};
    ($lit:literal) => {{ crate::sys::compat::const_cstr_from_bytes(concat!($lit, "\0").as_bytes()) }};
}

/// 在常量（const）上下文中，从字节切片创建一个 C 字符串封装。
///
/// 这是供 [`ansi_str`] 宏使用的一个工具函数。
///
/// # Panics
///
/// 如果切片不是以 null 结尾，或者除了最后一项之外还含有 null，则会 panic。
pub(crate) const fn const_cstr_from_bytes(bytes: &'static [u8]) -> &'static CStr {
    if !matches!(bytes.last(), Some(&0)) {
        panic!("A CStr must be null terminated");
    }
    let mut i = 0;
    // 到这里时 `len()` 至少为 1。
    while i < bytes.len() - 1 {
        if bytes[i] == 0 {
            panic!("A CStr must not have interior nulls")
        }
        i += 1;
    }
    // SAFETY: 安全性由上面的检查保证。
    unsafe { crate::ffi::CStr::from_bytes_with_nul_unchecked(bytes) }
}

/// 表示一个已加载的模块。
///
/// 注意：std 所依赖的模块绝不能被卸载。因此 `Module` 在 std 的整个生命周期内
/// 始终是有效的。
#[derive(Copy, Clone)]
pub(in crate::sys) struct Module(NonNull<c_void>);
impl Module {
    /// 尝试获取一个指向已加载模块的句柄。
    ///
    /// # SAFETY
    ///
    /// 这只应当用于在 std 整个生命周期内都存在的模块（例如 kernel32 和 ntdll）。
    pub unsafe fn new(name: &CStr) -> Option<Self> {
        // SAFETY: CStr 总是以 null 结尾的。
        unsafe {
            let module = c::GetModuleHandleA(name.as_ptr().cast::<u8>());
            NonNull::new(module).map(Self)
        }
    }

    // 尝试获取某个函数的地址。
    pub fn proc_address(self, name: &CStr) -> Option<NonNull<c_void>> {
        unsafe {
            // SAFETY:
            // `self.0` 始终是一个有效的模块。
            // CStr 总是以 null 结尾的。
            let proc = c::GetProcAddress(self.0.as_ptr(), name.as_ptr().cast::<u8>());
            // SAFETY: `GetProcAddress` 在为空时返回 None。
            proc.map(|p| NonNull::new_unchecked(p as *mut c_void))
        }
    }
}

/// 加载某个函数，如果加载失败则使用后备（fallback）实现。
macro_rules! compat_fn_with_fallback {
    (pub static $module:ident: &CStr = $name:expr; $(
        $(#[$meta:meta])*
        $vis:vis fn $symbol:ident($($argname:ident: $argtype:ty),*) -> $rettype:ty $fallback_body:block
    )*) => (
        pub static $module: &CStr = $name;
    $(
        $(#[$meta])*
        pub mod $symbol {
            #[allow(unused_imports)]
            use super::*;
            use crate::mem;
            use crate::ffi::CStr;
            use crate::sync::atomic::{Atomic, AtomicPtr, Ordering};
            use crate::sys::compat::Module;

            type F = unsafe extern "system" fn($($argtype),*) -> $rettype;

            /// `PTR` 中存放着指向三个函数之一的函数指针。
            /// 它起初指向 `load` 函数。
            /// 当 `load` 被调用时，它会尝试加载所请求的符号。
            /// 如果成功，`PTR` 被设置为该符号的地址。
            /// 如果失败，`PTR` 被设置为 `fallback`。
            static PTR: Atomic<*mut c_void> = AtomicPtr::new(load as unsafe extern "system" fn($($argname: $argtype),*) -> $rettype as *mut _);

            unsafe extern "system" fn load($($argname: $argtype),*) -> $rettype {
                unsafe {
                    let func = load_from_module(Module::new($module));
                    func($($argname),*)
                }
            }

            fn load_from_module(module: Option<Module>) -> F {
                unsafe {
                    static SYMBOL_NAME: &CStr = ansi_str!(sym $symbol);
                    if let Some(f) = module.and_then(|m| m.proc_address(SYMBOL_NAME)) {
                        PTR.store(f.as_ptr(), Ordering::Relaxed);
                        mem::transmute(f)
                    } else {
                        PTR.store(fallback as unsafe extern "system" fn($($argname: $argtype),*) -> $rettype as *mut _, Ordering::Relaxed);
                        fallback
                    }
                }
            }

            #[allow(unused_variables)]
            unsafe extern "system" fn fallback($($argname: $argtype),*) -> $rettype {
                $fallback_body
            }

            #[inline(always)]
            pub unsafe fn call($($argname: $argtype),*) -> $rettype {
                unsafe {
                    let func: F = mem::transmute(PTR.load(Ordering::Relaxed));
                    func($($argname),*)
                }
            }
        }
        #[allow(unused)]
        $(#[$meta])*
        $vis use $symbol::call as $symbol;
    )*)
}

/// 可选加载的函数。
///
/// 依赖于这些函数已在别处被预加载。
#[cfg(target_vendor = "win7")]
macro_rules! compat_fn_optional {
    ($(
        $(#[$meta:meta])*
        $vis:vis fn $symbol:ident($($argname:ident: $argtype:ty),*) $(-> $rettype:ty)?;
    )+) => (
        $(
            pub mod $symbol {
                #[allow(unused_imports)]
                use super::*;
                use crate::ffi::c_void;
                use crate::mem;
                use crate::ptr::{self, NonNull};
                use crate::sync::atomic::{Atomic, AtomicPtr, Ordering};

                pub(in crate::sys) static PTR: Atomic<*mut c_void> = AtomicPtr::new(ptr::null_mut());

                type F = unsafe extern "system" fn($($argtype),*) $(-> $rettype)?;

                #[inline(always)]
                pub fn option() -> Option<F> {
                    NonNull::new(PTR.load(Ordering::Relaxed)).map(|f| unsafe { mem::transmute(f) })
                }
            }
            #[inline]
            pub unsafe extern "system" fn $symbol($($argname: $argtype),*) $(-> $rettype)? {
                unsafe { $symbol::option().unwrap()($($argname),*) }
            }
        )+
    )
}

/// 从 "api-ms-win-core-synch-l1-2-0" 加载所有需要的函数。
#[cfg(target_vendor = "win7")]
pub(super) fn load_synch_functions() {
    fn try_load() -> Option<()> {
        use crate::sync::atomic::Ordering;
        const MODULE_NAME: &CStr = c"api-ms-win-core-synch-l1-2-0";
        const WAIT_ON_ADDRESS: &CStr = c"WaitOnAddress";
        const WAKE_BY_ADDRESS_SINGLE: &CStr = c"WakeByAddressSingle";

        // 尝试加载该库以及所有必需的函数。
        // 如果其中任何一步失败，则整体视为失败。
        let library = unsafe { Module::new(MODULE_NAME) }?;
        let wait_on_address = library.proc_address(WAIT_ON_ADDRESS)?;
        let wake_by_address_single = library.proc_address(WAKE_BY_ADDRESS_SINGLE)?;

        c::WaitOnAddress::PTR.store(wait_on_address.as_ptr(), Ordering::Relaxed);
        c::WakeByAddressSingle::PTR.store(wake_by_address_single.as_ptr(), Ordering::Relaxed);
        Some(())
    }

    try_load();
}
