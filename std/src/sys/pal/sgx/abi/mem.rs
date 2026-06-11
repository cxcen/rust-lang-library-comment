use core::arch::asm;

// 不要移除 inline：否则会导致重定位失败
#[inline(always)]
pub(crate) unsafe fn rel_ptr<T>(offset: u64) -> *const T {
    (image_base() + offset) as *const T
}

// 不要移除 inline：否则会导致重定位失败
#[inline(always)]
pub(crate) unsafe fn rel_ptr_mut<T>(offset: u64) -> *mut T {
    (image_base() + offset) as *mut T
}

unsafe extern "C" {
    static ENCLAVE_SIZE: usize;
    static HEAP_BASE: u64;
    static HEAP_SIZE: usize;
}

/// 返回堆的基础内存地址
pub(crate) fn heap_base() -> *const u8 {
    unsafe { rel_ptr_mut(HEAP_BASE) }
}

/// 返回堆的大小
pub(crate) fn heap_size() -> usize {
    unsafe { HEAP_SIZE }
}

// 不要移除 inline：否则会导致重定位失败
// 出于同样的原因，这里使用内联 ASM 而非 extern static 来定位 base
/// 返回当前 enclave 被加载到的地址。
#[inline(always)]
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn image_base() -> u64 {
    let base: u64;
    unsafe {
        asm!(
            "lea IMAGE_BASE(%rip), {}",
            lateout(reg) base,
            options(att_syntax, nostack, preserves_flags, nomem, pure),
        )
    };
    base
}

/// 如果指定的内存范围位于 enclave 内，则返回 `true`。
///
/// 出于安全考虑，此函数还会检查给定的范围是否溢出，若溢出则返回 `false`。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn is_enclave_range(p: *const u8, len: usize) -> bool {
    let start = p as usize;

    // 计算 `end` 时从 `len` 中减去 1，以防 `p + len` 恰好处于可寻址内存的末尾
    // （`p + len` 会溢出，但该范围仍然有效）。
    let end = if len == 0 {
        start
    } else if let Some(end) = start.checked_add(len - 1) {
        end
    } else {
        return false;
    };

    let base = image_base() as usize;
    start >= base && end <= base + (unsafe { ENCLAVE_SIZE } - 1) // unsafe ok: 链接期常量
}

/// 如果指定的内存范围位于用户空间（userspace）内，则返回 `true`。
///
/// 出于安全考虑，此函数还会检查给定的范围是否溢出，若溢出则返回 `false`。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn is_user_range(p: *const u8, len: usize) -> bool {
    let start = p as usize;

    // 计算 `end` 时从 `len` 中减去 1，以防 `p + len` 恰好处于可寻址内存的末尾
    // （`p + len` 会溢出，但该范围仍然有效）。
    let end = if len == 0 {
        start
    } else if let Some(end) = start.checked_add(len - 1) {
        end
    } else {
        return false;
    };

    let base = image_base() as usize;
    end < base || start > base + (unsafe { ENCLAVE_SIZE } - 1) // unsafe ok: 链接期常量
}
