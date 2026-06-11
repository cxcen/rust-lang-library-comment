//! 由 libgcc/libunwind（以某种形式）支撑的 panic 实现。
//!
//! 关于异常处理与栈展开（stack unwinding）的背景知识，请参见
//! “Exception Handling in LLVM”（llvm.org/docs/ExceptionHandling.html）
//! 以及从中链接出的各份文档。
//! 以下这些资料也值得一读：
//!  * <https://itanium-cxx-abi.github.io/cxx-abi/abi-eh.html>
//!  * <https://nicolasbrailo.github.io/blog/projects_texts/13exceptionsunderthehood.html>
//!  * <https://www.airs.com/blog/index.php?s=exception+frames>
//!
//! ## 简要概述
//!
//! 异常处理分两个阶段进行：搜索阶段（search phase）和清理阶段（cleanup phase）。
//!
//! 在这两个阶段中，unwinder（展开器）都会利用当前进程各模块的栈帧展开节
//!（stack frame unwind sections）中的信息，自顶向下遍历栈帧
//!（这里的“模块”指的是 OS 模块，即一个可执行文件或一个动态库）。
//!
//! 对于每个栈帧，它都会调用与之关联的 “personality routine”（personality 例程），
//! 其地址同样存储在展开信息节（unwind info section）中。
//!
//! 在搜索阶段，personality 例程的职责是检查正在被抛出的异常对象，
//! 并决定该异常是否应在那个栈帧处被捕获。一旦确定了处理者栈帧
//!（handler frame），清理阶段便开始。
//!
//! 在清理阶段，unwinder 会再次调用每个 personality 例程。这一次它会决定
//! 当前栈帧需要运行哪些清理代码（如果有的话）。如果需要，控制流会被转移到
//! 函数体中的一个特殊分支，即“落地区（landing pad）”，由它来调用析构函数、
//! 释放内存等。在落地区的末尾，控制流被转回 unwinder，展开继续进行。
//!
//! 一旦栈被展开到处理者栈帧那一层，展开便停止，最后一个 personality 例程
//! 把控制流转移到 catch 块。
#![forbid(unsafe_op_in_unsafe_fn)]

use unwind as uw;

use super::dwarf::eh::{self, EHAction, EHContext};
use crate::ffi::c_int;

// 这些寄存器 id 取自 LLVM 中各架构的 TargetLowering::getExceptionPointerRegister()
// 和 TargetLowering::getExceptionSelectorRegister()，然后通过寄存器定义表
//（通常是 <arch>RegisterInfo.td，搜索 "DwarfRegNum"）映射为 DWARF 寄存器号。
// 另见 https://llvm.org/docs/WritingAnLLVMBackend.html#defining-a-register 。

#[cfg(target_arch = "x86")]
const UNWIND_DATA_REG: (i32, i32) = (0, 2); // EAX, EDX

#[cfg(target_arch = "x86_64")]
const UNWIND_DATA_REG: (i32, i32) = (0, 1); // RAX, RDX

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
const UNWIND_DATA_REG: (i32, i32) = (0, 1); // R0, R1 / X0, X1

#[cfg(target_arch = "m68k")]
const UNWIND_DATA_REG: (i32, i32) = (0, 1); // D0, D1

#[cfg(any(
    target_arch = "mips",
    target_arch = "mips32r6",
    target_arch = "mips64",
    target_arch = "mips64r6"
))]
const UNWIND_DATA_REG: (i32, i32) = (4, 5); // A0, A1

#[cfg(target_arch = "csky")]
const UNWIND_DATA_REG: (i32, i32) = (0, 1); // R0, R1

#[cfg(any(target_arch = "powerpc", target_arch = "powerpc64"))]
const UNWIND_DATA_REG: (i32, i32) = (3, 4); // R3, R4 / X3, X4

#[cfg(target_arch = "s390x")]
const UNWIND_DATA_REG: (i32, i32) = (6, 7); // R6, R7

#[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
const UNWIND_DATA_REG: (i32, i32) = (24, 25); // I0, I1

#[cfg(target_arch = "hexagon")]
const UNWIND_DATA_REG: (i32, i32) = (0, 1); // R0, R1

#[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
const UNWIND_DATA_REG: (i32, i32) = (10, 11); // x10, x11

#[cfg(any(target_arch = "loongarch32", target_arch = "loongarch64"))]
const UNWIND_DATA_REG: (i32, i32) = (4, 5); // a0, a1

// 以下代码基于 GCC 的 C 和 C++ personality 例程。参考资料如下：
// https://github.com/gcc-mirror/gcc/blob/master/libstdc++-v3/libsupc++/eh_personality.cc
// https://github.com/gcc-mirror/gcc/blob/trunk/libgcc/unwind-c.c

cfg_select! {
    all(
        target_arch = "arm",
        not(target_vendor = "apple"),
        not(target_os = "netbsd"),
    ) => {
        /// 由 [ARM EHABI][armeabi-eh] 调用的 personality 函数
        ///
        /// iOS/tvOS/watchOS 上的 32 位 ARM 并不使用 ARM EHABI，它使用的是
        /// “setjmp-longjmp” 展开或 DWARF CFI 展开，这两者由默认例程处理。
        ///
        /// [armeabi-eh]: https://web.archive.org/web/20190728160938/https://infocenter.arm.com/help/topic/com.arm.doc.ihi0038b/IHI0038B_ehabi.pdf
        #[lang = "eh_personality"]
        unsafe extern "C" fn rust_eh_personality(
            state: uw::_Unwind_State,
            exception_object: *mut uw::_Unwind_Exception,
            context: *mut uw::_Unwind_Context,
        ) -> uw::_Unwind_Reason_Code {
            unsafe {
                let state = state as c_int;
                let action = state & uw::_US_ACTION_MASK as c_int;
                let search_phase = if action == uw::_US_VIRTUAL_UNWIND_FRAME as c_int {
                    // 在 ARM 上，backtrace 会以 state == _US_VIRTUAL_UNWIND_FRAME | _US_FORCE_UNWIND
                    // 来调用 personality 例程。在那些情况下，我们希望继续展开栈，
                    // 否则我们所有的 backtrace 都会终止在 __rust_try 处
                    if state & uw::_US_FORCE_UNWIND as c_int != 0 {
                        return continue_unwind(exception_object, context);
                    }
                    true
                } else if action == uw::_US_UNWIND_FRAME_STARTING as c_int {
                    false
                } else if action == uw::_US_UNWIND_FRAME_RESUME as c_int {
                    return continue_unwind(exception_object, context);
                } else {
                    return uw::_URC_FAILURE;
                };

                // DWARF unwinder 假定 _Unwind_Context 中保存着诸如函数指针和
                // LSDA 指针之类的东西，然而 ARM EHABI 却把它们放进了异常对象里。
                // 为了保持像 _Unwind_GetLanguageSpecificData() 这类只接收上下文指针的
                // 函数的签名不变，GCC 的 personality 例程会把一个指向 exception_object
                // 的指针藏在上下文中，使用为 ARM 的“暂存寄存器（scratch register）”(r12)
                // 保留的位置。
                uw::_Unwind_SetGR(context, uw::UNWIND_POINTER_REG, exception_object as uw::_Unwind_Ptr);
                // ……一种更有原则的做法是：在我们的 libunwind 绑定中提供 ARM 的
                // _Unwind_Context 的完整定义，直接从那里取出所需数据，绕开 DWARF
                // 兼容函数。

                let eh_action = match find_eh_action(context) {
                    Ok(action) => action,
                    Err(_) => return uw::_URC_FAILURE,
                };
                if search_phase {
                    match eh_action {
                        EHAction::None | EHAction::Cleanup(_) => {
                            return continue_unwind(exception_object, context);
                        }
                        EHAction::Catch(_) | EHAction::Filter(_) => {
                            // EHABI 要求 personality 例程更新异常对象的屏障缓存
                            //（barrier cache）中的 SP 值。
                            (*exception_object).private[5] =
                                uw::_Unwind_GetGR(context, uw::UNWIND_SP_REG);
                            return uw::_URC_HANDLER_FOUND;
                        }
                        EHAction::Terminate => return uw::_URC_FAILURE,
                    }
                } else {
                    match eh_action {
                        EHAction::None => return continue_unwind(exception_object, context),
                        EHAction::Filter(_) if state & uw::_US_FORCE_UNWIND as c_int != 0 => return continue_unwind(exception_object, context),
                        EHAction::Cleanup(lpad) | EHAction::Catch(lpad) | EHAction::Filter(lpad) => {
                            uw::_Unwind_SetGR(
                                context,
                                UNWIND_DATA_REG.0,
                                exception_object as uw::_Unwind_Ptr,
                            );
                            uw::_Unwind_SetGR(context, UNWIND_DATA_REG.1, core::ptr::null());
                            uw::_Unwind_SetIP(context, lpad);
                            return uw::_URC_INSTALL_CONTEXT;
                        }
                        EHAction::Terminate => return uw::_URC_FAILURE,
                    }
                }

                // 在 ARM EHABI 上，personality 例程负责在返回之前实际展开
                // 单个栈帧（ARM EHABI 第 6.1 节）。
                unsafe fn continue_unwind(
                    exception_object: *mut uw::_Unwind_Exception,
                    context: *mut uw::_Unwind_Context,
                ) -> uw::_Unwind_Reason_Code {
                    unsafe {
                        if __gnu_unwind_frame(exception_object, context) == uw::_URC_NO_REASON {
                            uw::_URC_CONTINUE_UNWIND
                        } else {
                            uw::_URC_FAILURE
                        }
                    }
                }
                // 定义于 libgcc 中
                unsafe extern "C" {
                    fn __gnu_unwind_frame(
                        exception_object: *mut uw::_Unwind_Exception,
                        context: *mut uw::_Unwind_Context,
                    ) -> uw::_Unwind_Reason_Code;
                }
            }
        }
    }
    _ => {
        /// 默认的 personality 例程，它在大多数 target 上被直接使用，
        /// 并在 Windows x86_64 与 AArch64 上经由 SEH 间接使用。
        unsafe extern "C" fn rust_eh_personality_impl(
            version: c_int,
            actions: uw::_Unwind_Action,
            _exception_class: uw::_Unwind_Exception_Class,
            exception_object: *mut uw::_Unwind_Exception,
            context: *mut uw::_Unwind_Context,
        ) -> uw::_Unwind_Reason_Code {
            unsafe {
                if version != 1 {
                    return uw::_URC_FATAL_PHASE1_ERROR;
                }
                let eh_action = match find_eh_action(context) {
                    Ok(action) => action,
                    Err(_) => return uw::_URC_FATAL_PHASE1_ERROR,
                };
                if actions & uw::_UA_SEARCH_PHASE != 0 {
                    match eh_action {
                        EHAction::None | EHAction::Cleanup(_) => uw::_URC_CONTINUE_UNWIND,
                        EHAction::Catch(_) | EHAction::Filter(_) => uw::_URC_HANDLER_FOUND,
                        EHAction::Terminate => uw::_URC_FATAL_PHASE1_ERROR,
                    }
                } else {
                    match eh_action {
                        EHAction::None => uw::_URC_CONTINUE_UNWIND,
                        // 强制展开（forced unwinding）遇到了一个 terminate 动作。
                        EHAction::Filter(_) if actions & uw::_UA_FORCE_UNWIND != 0 => uw::_URC_CONTINUE_UNWIND,
                        EHAction::Cleanup(lpad) | EHAction::Catch(lpad) | EHAction::Filter(lpad) => {
                            uw::_Unwind_SetGR(
                                context,
                                UNWIND_DATA_REG.0,
                                exception_object.cast(),
                            );
                            uw::_Unwind_SetGR(context, UNWIND_DATA_REG.1, core::ptr::null());
                            uw::_Unwind_SetIP(context, lpad);
                            uw::_URC_INSTALL_CONTEXT
                        }
                        EHAction::Terminate => uw::_URC_FATAL_PHASE2_ERROR,
                    }
                }
            }
        }

        cfg_select! {
            any(
                all(windows, any(target_arch = "aarch64", target_arch = "x86_64"), target_env = "gnu"),
                target_os = "cygwin",
            ) => {
                /// 由 [Windows 结构化异常处理（Structured Exception Handling）][windows-eh]
                /// 调用的 personality 函数
                ///
                /// 在 x86_64 和 AArch64 MinGW target 上，展开机制是 SEH，
                /// 然而展开处理者数据（即 LSDA）使用的是 GCC 兼容的编码
                ///
                /// [windows-eh]: https://learn.microsoft.com/en-us/cpp/cpp/structured-exception-handling-c-cpp?view=msvc-170
                #[lang = "eh_personality"]
                #[allow(nonstandard_style)]
                unsafe extern "C" fn rust_eh_personality(
                    exceptionRecord: *mut uw::EXCEPTION_RECORD,
                    establisherFrame: uw::LPVOID,
                    contextRecord: *mut uw::CONTEXT,
                    dispatcherContext: *mut uw::DISPATCHER_CONTEXT,
                ) -> uw::EXCEPTION_DISPOSITION {
                    // SAFETY：cfg 仍然是 target_os = "windows" 且 target_env = "gnu"，
                    // 这意味着这是要调用的正确函数，并把我们的 impl 函数作为实际被使用的
                    // 回调传入
                    unsafe {
                        uw::_GCC_specific_handler(
                            exceptionRecord,
                            establisherFrame,
                            contextRecord,
                            dispatcherContext,
                            rust_eh_personality_impl,
                        )
                    }
                }
            }
            _ => {
                /// 由 [Itanium C++ ABI 异常处理][itanium-eh] 调用的 personality 函数
                ///
                /// 这是大多数非 Windows target 的 personality 例程。它会被展开库调用：
                /// - “在搜索阶段，框架会反复调用 personality 例程，并附带下文所述的
                ///   _UA_SEARCH_PHASE 标志，先针对当前的 PC 和寄存器状态调用，然后在每一步
                ///   把一帧展开到一个新的 PC……”
                /// - “如果搜索阶段报告成功，框架便在清理阶段重新开始。同样，它会反复调用
                ///   personality 例程，并附带下文所述的 _UA_CLEANUP_PHASE 标志，先针对当前的
                ///   PC 和寄存器状态调用，然后在每一步把一帧展开到一个新的 PC……”
                ///
                /// [itanium-eh]: https://itanium-cxx-abi.github.io/cxx-abi/abi-eh.html
                #[lang = "eh_personality"]
                unsafe extern "C" fn rust_eh_personality(
                    version: c_int,
                    actions: uw::_Unwind_Action,
                    exception_class: uw::_Unwind_Exception_Class,
                    exception_object: *mut uw::_Unwind_Exception,
                    context: *mut uw::_Unwind_Context,
                ) -> uw::_Unwind_Reason_Code {
                    // SAFETY：如果平台支持需要与当前所调用的不同的东西，
                    // 它必须为内部 fn 修改 cfg。
                    unsafe {
                        rust_eh_personality_impl(
                            version,
                            actions,
                            exception_class,
                            exception_object,
                            context,
                        )
                    }
                }
            }
        }
    }
}

unsafe fn find_eh_action(context: *mut uw::_Unwind_Context) -> Result<EHAction, ()> {
    unsafe {
        let lsda = uw::_Unwind_GetLanguageSpecificData(context) as *const u8;
        let mut ip_before_instr: c_int = 0;
        let ip = uw::_Unwind_GetIPInfo(context, &mut ip_before_instr);
        let eh_context = EHContext {
            // 返回地址指向调用指令之后的 1 个字节处，它可能落在 LSDA 范围表
            //（range table）中的下一个 IP 区间里。
            //
            // `ip = -1` 有特殊含义，所以使用 wrapping sub 来允许这种情况
            ip: if ip_before_instr != 0 { ip } else { ip.wrapping_sub(1) },
            func_start: uw::_Unwind_GetRegionStart(context),
            get_text_start: &|| uw::_Unwind_GetTextRelBase(context),
            get_data_start: &|| uw::_Unwind_GetDataRelBase(context),
        };
        eh::find_eh_action(lsda, &eh_context)
    }
}
