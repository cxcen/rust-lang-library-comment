//! SGX 平台特有的、对体系结构特性的访问。
//!
//! 本模块中的功能在 Intel Software Developer's Manual, Volume 3, Chapter 40 中有进一步的说明。
#![unstable(feature = "sgx_platform", issue = "56975")]

use core::arch::asm;

use crate::mem::MaybeUninit;

/// 用于强制 16 字节对齐的包装结构体。
#[repr(align(16))]
#[unstable(feature = "sgx_platform", issue = "56975")]
pub struct Align16<T>(pub T);

/// 用于强制 128 字节对齐的包装结构体。
#[repr(align(128))]
#[unstable(feature = "sgx_platform", issue = "56975")]
pub struct Align128<T>(pub T);

/// 用于强制 512 字节对齐的包装结构体。
#[repr(align(512))]
#[unstable(feature = "sgx_platform", issue = "56975")]
pub struct Align512<T>(pub T);

const ENCLU_EREPORT: u32 = 0;
const ENCLU_EGETKEY: u32 = 1;

/// 调用 `EGETKEY` 指令以获取一个 128 位的密钥。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn egetkey(request: &Align512<[u8; 512]>) -> Result<Align16<[u8; 16]>, u32> {
    unsafe {
        let mut out = MaybeUninit::uninit();
        let error;

        asm!(
            // rbx 被 LLVM 保留
            "xchg %rbx, {0}",
            "enclu",
            "mov {0}, %rbx",
            inout(reg) request => _,
            inlateout("eax") ENCLU_EGETKEY => error,
            in("rcx") out.as_mut_ptr(),
            options(att_syntax, nostack),
        );

        match error {
            0 => Ok(out.assume_init()),
            err => Err(err),
        }
    }
}

/// 调用 `EREPORT` 指令。
///
/// 这会创建一份描述当前 enclave 内容的加密报告（cryptographic report）。该报告可由
/// `targetinfo` 所描述的 enclave 进行验证。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn ereport(
    targetinfo: &Align512<[u8; 512]>,
    reportdata: &Align128<[u8; 64]>,
) -> Align512<[u8; 432]> {
    unsafe {
        let mut report = MaybeUninit::uninit();

        asm!(
            // rbx 被 LLVM 保留
            "xchg %rbx, {0}",
            "enclu",
            "mov {0}, %rbx",
            inout(reg) targetinfo => _,
            in("eax") ENCLU_EREPORT,
            in("rcx") reportdata,
            in("rdx") report.as_mut_ptr(),
            options(att_syntax, preserves_flags, nostack),
        );

        report.assume_init()
    }
}
