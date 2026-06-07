//! C 的“可变参数”
//!
//! 通常称为 "varargs"。

#[cfg(not(target_arch = "xtensa"))]
use crate::ffi::c_void;
use crate::fmt;
use crate::intrinsics::{va_arg, va_copy};
use crate::marker::PhantomCovariantLifetime;

// 对 Rust 支持的目标,C `va_list` 目前有三种实现形式:
//
// - `va_list` 是不透明指针
// - `va_list` 是结构体
// - `va_list` 是包含一个结构体的单元素数组
//
// 不透明指针方案最容易实现:该指针只是指向调用方栈上的参数数组。
//
// 结构体和单元素数组变体更复杂,但可能更高效,因为额外状态让通过寄存器传递可变参数成为可能。
//
// Rust 的 `VaList` 类型与 C 的 `va_list` ABI 兼容。结构体和指针情形可直接映射到
// 对应 Rust 形式,但单元素数组情形比较特殊:在 C 中,该类型会发生 array-to-pointer decay。
//
// `#[rustc_pass_indirectly_in_non_rustic_abis]` 属性用于在 Rust 中匹配这种指针退化行为,
// 同时在其他方面保持 Rust 语义。该属性确保编译器对
// `extern "C" fn takes_va_list(va: VaList<'_>)` 这类函数使用正确 ABI,即间接传递 `va`。
//
// Clang 的 `BuiltinVaListKind` 枚举了 Clang 支持的 `va_list` 变体,这里保持对应。
crate::cfg_select! {
    all(
        target_arch = "aarch64",
        not(target_vendor = "apple"),
        not(target_os = "uefi"),
        not(windows),
    ) => {
        /// `va_list` 的 AArch64 ABI 实现。
        ///
        /// 更多细节见 [AArch64 Procedure Call Standard]。
        ///
        /// [AArch64 Procedure Call Standard]:
        /// http://infocenter.arm.com/help/topic/com.arm.doc.ihi0055b/IHI0055B_aapcs64.pdf
        #[repr(C)]
        #[derive(Debug)]
        struct VaListInner {
            stack: *const c_void,
            gr_top: *const c_void,
            vr_top: *const c_void,
            gr_offs: i32,
            vr_offs: i32,
        }
    }
    all(target_arch = "powerpc", not(target_os = "uefi"), not(windows)) => {
        /// `va_list` 的 PowerPC ABI 实现。
        ///
        /// 更多细节见 [LLVM source] 和 [GCC header]。
        ///
        /// [LLVM source]:
        /// https://github.com/llvm/llvm-project/blob/af9a4263a1a209953a1d339ef781a954e31268ff/llvm/lib/Target/PowerPC/PPCISelLowering.cpp#L4089-L4111
        /// [GCC header]: https://web.mit.edu/darwin/src/modules/gcc/gcc/ginclude/va-ppc.h
        #[repr(C)]
        #[derive(Debug)]
        #[rustc_pass_indirectly_in_non_rustic_abis]
        struct VaListInner {
            gpr: u8,
            fpr: u8,
            reserved: u16,
            overflow_arg_area: *const c_void,
            reg_save_area: *const c_void,
        }
    }
    target_arch = "s390x" => {
        /// `va_list` 的 s390x ABI 实现。
        ///
        /// 更多细节见 [S/390x ELF Application Binary Interface Supplement]。
        ///
        /// [S/390x ELF Application Binary Interface Supplement]:
        /// https://docs.google.com/gview?embedded=true&url=https://github.com/IBM/s390x-abi/releases/download/v1.7/lzsabi_s390x.pdf
        #[repr(C)]
        #[derive(Debug)]
        #[rustc_pass_indirectly_in_non_rustic_abis]
        struct VaListInner {
            gpr: i64,
            fpr: i64,
            overflow_arg_area: *const c_void,
            reg_save_area: *const c_void,
        }
    }
    all(target_arch = "x86_64", not(target_os = "uefi"), not(windows)) => {
        /// `va_list` 的 x86_64 System V ABI 实现。
        ///
        /// 更多细节见 [System V AMD64 ABI]。
        ///
        /// [System V AMD64 ABI]:
        /// https://refspecs.linuxbase.org/elf/x86_64-abi-0.99.pdf
        #[repr(C)]
        #[derive(Debug)]
        #[rustc_pass_indirectly_in_non_rustic_abis]
        struct VaListInner {
            gp_offset: i32,
            fp_offset: i32,
            overflow_arg_area: *const c_void,
            reg_save_area: *const c_void,
        }
    }
    target_arch = "xtensa" => {
        /// `va_list` 的 Xtensa ABI 实现。
        ///
        /// 更多细节见 [LLVM source]。
        ///
        /// [LLVM source]:
        /// https://github.com/llvm/llvm-project/blob/af9a4263a1a209953a1d339ef781a954e31268ff/llvm/lib/Target/Xtensa/XtensaISelLowering.cpp#L1211-L1215
        #[repr(C)]
        #[derive(Debug)]
        #[rustc_pass_indirectly_in_non_rustic_abis]
        struct VaListInner {
            stk: *const i32,
            reg: *const i32,
            ndx: i32,
        }
    }

    all(target_arch = "hexagon", target_env = "musl") => {
        /// `va_list` 的 Hexagon Musl 实现。
        ///
        /// 更多细节见 [LLVM source]。裸机 Hexagon 使用不透明指针。
        ///
        /// [LLVM source]:
        /// https://github.com/llvm/llvm-project/blob/0cdc1b6dd4a870fc41d4b15ad97e0001882aba58/clang/lib/CodeGen/Targets/Hexagon.cpp#L407-L417
        #[repr(C)]
        #[derive(Debug)]
        #[rustc_pass_indirectly_in_non_rustic_abis]
        struct VaListInner {
            __current_saved_reg_area_pointer: *const c_void,
            __saved_reg_area_end_pointer: *const c_void,
            __overflow_area_pointer: *const c_void,
        }
    }

    // fallback 实现,用于:
    //
    // - apple aarch64(见 https://github.com/rust-lang/rust/pull/56599)
    // - windows
    // - powerpc64 & powerpc64le
    // - uefi
    // - 任何其他未在上方指定 `VaListInner` 的目标
    //
    // 在该实现中,`va_list` 类型只是某个不透明指针的别名。
    // 该指针很可能指向调用方栈上的下一个可变参数。
    _ => {
        /// `va_list` 的基础实现。
        #[repr(transparent)]
        #[derive(Debug)]
        struct VaListInner {
            ptr: *const c_void,
        }
    }
}

/// 可变参数列表,等价于 C 中的 `va_list`。
#[repr(transparent)]
#[lang = "va_list"]
pub struct VaList<'a> {
    inner: VaListInner,
    _marker: PhantomCovariantLifetime<'a>,
}

impl fmt::Debug for VaList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // debug 输出中无需包含 `_marker`。
        f.debug_tuple("VaList").field(&self.inner).finish()
    }
}

mod sealed {
    pub trait Sealed {}

    impl Sealed for i32 {}
    impl Sealed for i64 {}
    impl Sealed for isize {}

    impl Sealed for u32 {}
    impl Sealed for u64 {}
    impl Sealed for usize {}

    impl Sealed for f64 {}

    impl<T> Sealed for *mut T {}
    impl<T> Sealed for *const T {}
}

/// 可通过 [`VaList::arg`] 合法读取的类型。
///
/// # 安全性(Safety）
///
/// 标准库只为那些预期在所有平台上都拥有可变参数应用二进制接口(ABI)的基本类型实现本 trait。
///
/// C 传递可变参数时,小于 [`c_int`] 的整数和小于 [`c_double`] 的浮点数会分别隐式提升为
/// [`c_int`] 和 [`c_double`]。为受该提升规则影响的类型实现本 trait 是无效的。
///
/// [`c_int`]: core::ffi::c_int
/// [`c_double`]: core::ffi::c_double
// 未来可能会解除本 trait 的 sealed 限制,但目前我们的 `va_arg` 实现不支持对齐大于 8
// 或布局不是标量的类型。在此期间,可使用内联汇编接收不受支持的类型。
pub unsafe trait VaArgSafe: sealed::Sealed {}

// i8 和 i16 在 C 中会隐式提升为 c_int,因此不能实现 `VaArgSafe`。
unsafe impl VaArgSafe for i32 {}
unsafe impl VaArgSafe for i64 {}
unsafe impl VaArgSafe for isize {}

// u8 和 u16 在 C 中会隐式提升为 c_int,因此不能实现 `VaArgSafe`。
unsafe impl VaArgSafe for u32 {}
unsafe impl VaArgSafe for u64 {}
unsafe impl VaArgSafe for usize {}

// f32 在 C 中会隐式提升为 c_double,因此不能实现 `VaArgSafe`。
unsafe impl VaArgSafe for f64 {}

unsafe impl<T> VaArgSafe for *mut T {}
unsafe impl<T> VaArgSafe for *const T {}

impl<'f> VaList<'f> {
    /// 前进到下一个可变参数并读取它。
    ///
    /// # 安全性(Safety）
    ///
    /// 只有满足以下条件时,调用本函数才是健全的:
    ///
    /// - 确实存在下一个可变参数。
    /// - 下一个参数的类型必须与类型 `T` ABI 兼容。
    /// - 下一个参数必须包含正确初始化的 `T` 类型值。
    ///
    /// 若类型不兼容、值无效,或已经没有更多可变参数,调用本函数都是不健全的。
    ///
    /// [valid]: https://doc.rust-lang.org/nightly/nomicon/what-unsafe-does.html
    #[inline]
    pub unsafe fn arg<T: VaArgSafe>(&mut self) -> T {
        // SAFETY: 调用方必须维护 `va_arg` 的安全契约。
        unsafe { va_arg(self) }
    }
}

impl<'f> Clone for VaList<'f> {
    #[inline]
    fn clone(&self) -> Self {
        let mut dest = crate::mem::MaybeUninit::uninit();
        // SAFETY: 我们会写入 `MaybeUninit`,因此它已初始化,调用 `assume_init` 合法。
        unsafe {
            va_copy(dest.as_mut_ptr(), self);
            dest.assume_init()
        }
    }
}

impl<'f> Drop for VaList<'f> {
    fn drop(&mut self) {
        // Rust 要求对 `va_list` 不调用 `va_end` 不会导致未定义行为(泄漏值是安全的)。
        // 由于 `va_end` 在所有当前 LLVM 目标上都是 no-op,该析构器为空。
    }
}

// 通过 `compiler/rustc_ty_utils/src/abi.rs` 中的 assert 检查当前目标的 C ABI
// 是否正确实现 `rustc_pass_indirectly_in_non_rustic_abis`。
const _: () = {
    #[repr(C)]
    #[rustc_pass_indirectly_in_non_rustic_abis]
    struct Type(usize);

    const extern "C" fn c(_: Type) {}

    c(Type(0))
};
