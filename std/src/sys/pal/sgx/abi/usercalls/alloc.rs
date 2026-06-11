#![allow(unused)]

use fortanix_sgx_abi::*;

use super::super::mem::{is_enclave_range, is_user_range};
use crate::arch::asm;
use crate::cell::UnsafeCell;
use crate::convert::TryInto;
use crate::mem::{self, ManuallyDrop, MaybeUninit};
use crate::ops::{CoerceUnsized, Deref, DerefMut, Index, IndexMut};
use crate::pin::PinCoerceUnsized;
use crate::ptr::{self, NonNull};
use crate::slice::SliceIndex;
use crate::{cmp, intrinsics, slice};

/// 一种可以安全地从用户空间（userspace）读取、或写入到用户空间的类型。
///
/// 读取与写入的具体要求（非穷尽列表）：
/// * **类型为 `Copy`**（因此也不是 `Drop`）。从/向用户空间复制时会创建副本。
///   不会调用析构函数。
/// * **没有引用或 Rust 风格的拥有型指针**（`Vec`、`Arc` 等）。从用户空间读取时，
///   绝不能创建指向 enclave 内存的引用。此外，只有 enclave 内存才被视为受 Rust
///   编译器静态分析管理。从用户空间读取时，无法保证该值正确符合该类型的预期。
///   写入到用户空间时，出于机密性原因，绝不能泄漏 enclave 内存中数据的内存地址。
///   出于同样的原因，`User` 和 `UserRef` 也是不允许的。
/// * **没有胖指针（fat pointer）。** 从用户空间读取时，代码可能会自动解释并使用
///   其中的大小或 vtable 指针。写入到用户空间时，出于机密性原因，绝不能泄漏
///   enclave 内存中数据的内存地址（例如 vtable 指针）。
///
/// 从用户空间读取的具体要求（非穷尽列表）：
/// * **任意位模式（bit pattern）对该类型都有效**（不能有 `enum`）。无法保证该值
///   正确符合该类型的预期，因此任意值对该类型都必须是有效的。
///
/// 写入到用户空间的具体要求（非穷尽列表）：
/// * **没有指向 enclave 内存的指针。** 出于机密性原因，绝不能泄漏 enclave 内存中
///   数据的内存地址。
/// * **没有内部填充（padding）。** 填充字节可能包含先前存储在该内存位置的、已被
///   初始化的机密数据，出于机密性原因绝不能泄漏。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub unsafe trait UserSafeSized: Copy + Sized {}

#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl UserSafeSized for u8 {}
#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl<T> UserSafeSized for FifoDescriptor<T> {}
#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl UserSafeSized for ByteBuffer {}
#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl UserSafeSized for Usercall {}
#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl UserSafeSized for Return {}
#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl UserSafeSized for Cancel {}
#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl<T: UserSafeSized> UserSafeSized for [T; 2] {}

/// 一种可以在内存中表示为一个或多个 `UserSafeSized` 的类型。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub unsafe trait UserSafe {
    /// 等价于 `align_of::<Self>`。
    fn align_of() -> usize;

    /// 给定用户空间中的一段内存范围，构造一个指向 `Self` 的指针。
    ///
    /// 注意，它接收的是 size（字节大小），而非 length（元素个数）！
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须确保该内存范围位于用户内存中、大小正确、对齐正确，并指向正确的
    /// 类型。
    unsafe fn from_raw_sized_unchecked(ptr: *mut u8, size: usize) -> *mut Self;

    /// 给定一段内存范围，构造一个指向 `Self` 的指针。
    ///
    /// 注意，它接收的是 size（字节大小），而非 length（元素个数）！
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须确保该内存范围指向正确的类型。
    ///
    /// # Panics
    ///
    /// 此函数在以下情况下 panic：
    ///
    /// * 指针未对齐。
    /// * 指针为空。
    /// * 所指向的范围无法放进地址空间。
    /// * 所指向的范围不在用户内存中。
    unsafe fn from_raw_sized(ptr: *mut u8, size: usize) -> NonNull<Self> {
        assert!(ptr.wrapping_add(size) >= ptr);
        // SAFETY: 调用方已保证该指针有效
        let ret = unsafe { Self::from_raw_sized_unchecked(ptr, size) };
        unsafe {
            Self::check_ptr(ret);
            NonNull::new_unchecked(ret as _)
        }
    }

    /// 检查某个指针是否可能指向用户内存中的 `Self`。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须确保该内存范围指向正确的类型与长度（如果这是一个切片）。
    ///
    /// # Panics
    ///
    /// 此函数在以下情况下 panic：
    ///
    /// * 指针未对齐。
    /// * 指针为空。
    /// * 所指向的范围不在用户内存中。
    unsafe fn check_ptr(ptr: *const Self) {
        let is_aligned = |p: *const u8| -> bool { p.is_aligned_to(Self::align_of()) };

        assert!(is_aligned(ptr as *const u8));
        assert!(is_user_range(ptr as _, size_of_val(unsafe { &*ptr })));
        assert!(!ptr.is_null());
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl<T: UserSafeSized> UserSafe for T {
    fn align_of() -> usize {
        align_of::<T>()
    }

    unsafe fn from_raw_sized_unchecked(ptr: *mut u8, size: usize) -> *mut Self {
        assert_eq!(size, size_of::<T>());
        ptr as _
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl<T: UserSafeSized> UserSafe for [T] {
    fn align_of() -> usize {
        align_of::<T>()
    }

    /// # Panics
    ///
    /// 此函数在以下情况下 panic：
    ///
    /// * 元素大小不是该 size 的因数
    unsafe fn from_raw_sized_unchecked(ptr: *mut u8, size: usize) -> *mut Self {
        let elem_size = size_of::<T>();
        assert_eq!(size % elem_size, 0);
        let len = size / elem_size;
        ptr::slice_from_raw_parts_mut(ptr as _, len)
    }
}

/// 指向用户空间内存中某个类型的引用。`&UserRef<T>` 等价于 enclave 内存中的
/// `&T`。为避免 TOCTTOU（检查时与使用时不一致）问题，只允许通过复制来访问该
/// 内存。复制之后，代码应当在使用前对该值进行完整检查。
///
/// 也可以获得一个可变引用 `&mut UserRef<T>`。与常规可变引用不同，这些引用并非
/// 独占（exclusive）。用户空间随时都可能写入底层内存，因此不能假定所指向的内存
/// 是被唯一借用的。提供这两种不同的引用类型纯粹是为了表明意图：可变引用用于写入
/// 用户内存，不可变引用用于从用户内存读取。
#[unstable(feature = "sgx_platform", issue = "56975")]
#[repr(transparent)]
pub struct UserRef<T: ?Sized>(UnsafeCell<T>);
/// 用户空间内存中的一个拥有型（owned）类型。`User<T>` 等价于 enclave 内存中的
/// `Box<T>`。为避免 TOCTTOU 问题，只允许通过复制来访问该内存。当该值被 drop 时，
/// 这块用户内存会被释放。复制之后，代码应当在使用前对该值进行完整检查。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub struct User<T: UserSafe + ?Sized>(NonNull<UserRef<T>>);

#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl<T: UserSafeSized> Send for User<T> {}

#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl<T: UserSafeSized> Send for User<[T]> {}

trait NewUserRef<T: ?Sized> {
    unsafe fn new_userref(v: T) -> Self;
}

impl<T: ?Sized> NewUserRef<*mut T> for NonNull<UserRef<T>> {
    unsafe fn new_userref(v: *mut T) -> Self {
        // SAFETY: 调用方已保证该指针有效
        unsafe { NonNull::new_unchecked(v as _) }
    }
}

impl<T: ?Sized> NewUserRef<NonNull<T>> for NonNull<UserRef<T>> {
    unsafe fn new_userref(v: NonNull<T>) -> Self {
        // SAFETY: 调用方已保证该指针有效
        unsafe { NonNull::new_userref(v.as_ptr()) }
    }
}

/// 一种可以作为安全地从用户空间复制数据时的目标的类型。
///
/// # 安全性(Safety）
///
/// 要求 `T` 与 `Self` 具有完全相同的内存布局。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub unsafe trait UserSafeCopyDestination<T: ?Sized> {
    /// 返回一个用于向该值写入的指针。
    fn as_mut_ptr(&mut self) -> *mut T;
}

#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl<T> UserSafeCopyDestination<T> for T {
    fn as_mut_ptr(&mut self) -> *mut T {
        self as _
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl<T> UserSafeCopyDestination<[T]> for [T] {
    fn as_mut_ptr(&mut self) -> *mut [T] {
        self as _
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl<T> UserSafeCopyDestination<T> for MaybeUninit<T> {
    fn as_mut_ptr(&mut self) -> *mut T {
        self as *mut Self as _
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
unsafe impl<T> UserSafeCopyDestination<[T]> for [MaybeUninit<T>] {
    fn as_mut_ptr(&mut self) -> *mut [T] {
        self as *mut Self as _
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<T: ?Sized> User<T>
where
    T: UserSafe,
{
    // 此函数返回的内存实际上是未初始化的，但就优化编译器而言，它不被视为
    // “未指定（unspecified）”或“未定义（undefined）”。这是通过返回一个由
    // `super::alloc` 从外部获取的指针来实现的。
    fn new_uninit_bytes(size: usize) -> Self {
        unsafe {
            // 绝不能以 size 为 0 调用 alloc。
            let ptr = if size > 0 {
                // 当数据按 8 字节对齐时，`copy_to_userspace` 更高效
                let alignment = cmp::max(T::align_of(), 8);
                rtunwrap!(Ok, super::alloc(size, alignment)) as _
            } else {
                T::align_of() as _ // 对于 size 为 0，悬垂（dangling）指针是可以的
            };
            if let Ok(v) = crate::panic::catch_unwind(|| T::from_raw_sized(ptr, size)) {
                User(NonNull::new_userref(v))
            } else {
                rtabort!("Got invalid pointer from alloc() usercall")
            }
        }
    }

    /// 把 `val` 复制到用户内存中新分配的空间里。
    pub fn new_from_enclave(val: &T) -> Self {
        unsafe {
            let mut user = Self::new_uninit_bytes(size_of_val(val));
            user.copy_from_enclave(val);
            user
        }
    }

    /// 从一个裸指针创建一个拥有型的 `User<T>`。
    ///
    /// # 安全性(Safety）
    /// 调用方必须确保 `ptr` 指向 `T`、可用 `free` usercall 以及 `T` 的对齐方式
    /// 进行释放，且为唯一拥有（uniquely owned）。
    ///
    /// # Panics
    /// 此函数在以下情况下 panic：
    ///
    /// * 指针未对齐
    /// * 指针为空
    /// * 所指向的范围不在用户内存中
    pub unsafe fn from_raw(ptr: *mut T) -> Self {
        // SAFETY: 调用方必须遵守 `from_raw` 的安全契约。
        unsafe { T::check_ptr(ptr) };
        User(unsafe { NonNull::new_userref(ptr) })
    }

    /// 把该值转换为一个裸指针。该值将不再被自动释放。
    pub fn into_raw(self) -> *mut T {
        ManuallyDrop::new(self).0.as_ptr() as _
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<T> User<T>
where
    T: UserSafe,
{
    /// 在用户内存中为 `T` 分配空间。
    pub fn uninitialized() -> Self {
        Self::new_uninit_bytes(size_of::<T>())
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<T> User<[T]>
where
    [T]: UserSafe,
{
    /// 在用户内存中为含 `n` 个元素的 `[T]` 分配空间。
    pub fn uninitialized(n: usize) -> Self {
        Self::new_uninit_bytes(n * size_of::<T>())
    }

    /// 从一个裸瘦指针（thin pointer）和切片长度创建一个拥有型的 `User<[T]>`。
    ///
    /// # 安全性(Safety）
    /// 调用方必须确保 `ptr` 指向 `len` 个 `T` 类型的元素、可用 `free` usercall
    /// 以及 `T` 的对齐方式进行释放，且为唯一拥有（uniquely owned）。
    ///
    /// # Panics
    /// 此函数在以下情况下 panic：
    ///
    /// * 指针未对齐
    /// * 指针为空
    /// * 所指向的范围无法放进地址空间
    /// * 所指向的范围不在用户内存中
    pub unsafe fn from_raw_parts(ptr: *mut T, len: usize) -> Self {
        User(unsafe { NonNull::new_userref(<[T]>::from_raw_sized(ptr as _, len * size_of::<T>())) })
    }
}

/// 将切片 `(ptr, len)` 分成三部分，其中中间部分对齐到 `u64`。
///
/// 返回值 `(prefix_len, mid_len, suffix_len)` 相加后重新等于 `len`。返回值满足：
/// 内存区域 `(ptr + prefix_len, mid_len)` 是这样一个最大可能区域——其中
/// `ptr + prefix_len` 对齐到 `u64`，且 `mid_len` 是 `u64` 字节大小的整数倍。
/// 这意味着 `prefix_len` 和 `suffix_len` 都保证小于 `u64` 的字节大小，并且
/// `(ptr, prefix_len)` 与 `(ptr + prefix_len + mid_len, suffix_len)` 都不会跨越
/// 对齐边界。
// 标准的 Rust 函数（如 `<[u8]>::align_to::<u64>` 和 `<*const u8>::align_offset`）
// 并不_保证_计算出最大可能的中间区域，因此不能使用它们。
fn u64_align_to_guaranteed(ptr: *const u8, mut len: usize) -> (usize, usize, usize) {
    const QWORD_SIZE: usize = size_of::<u64>();

    let offset = ptr as usize % QWORD_SIZE;

    let prefix_len = if intrinsics::unlikely(offset > 0) { QWORD_SIZE - offset } else { 0 };

    len = match len.checked_sub(prefix_len) {
        Some(remaining_len) => remaining_len,
        None => return (len, 0, 0),
    };

    let suffix_len = len % QWORD_SIZE;
    len -= suffix_len;

    (prefix_len, len, suffix_len)
}

unsafe fn copy_quadwords(src: *const u8, dst: *mut u8, len: usize) {
    unsafe {
        asm!(
            "rep movsq (%rsi), (%rdi)",
            inout("rcx") len / 8 => _,
            inout("rdi") dst => _,
            inout("rsi") src => _,
            options(att_syntax, nostack, preserves_flags)
        );
    }
}

/// 把 `len` 字节的数据从 enclave 指针 `src` 复制到用户空间 `dst`
///
/// 此函数通过确保对不可信内存的所有写入满足以下条件之一，来缓解陈旧数据
/// （stale data）漏洞：
///  - 前置 VERW 指令、后接 MFENCE; LFENCE 指令序列
///  - 或者为 8 字节的整数倍，并对齐到 8 字节边界
///
/// # Panics
/// 此函数在以下情况下 panic：
///
/// * `src` 指针为空
/// * `dst` 指针为空
/// * `src` 内存范围不在 enclave 内存中
/// * `dst` 内存范围不在用户内存中
///
/// # 参考资料(References)
///  - https://www.intel.com/content/www/us/en/security-center/advisory/intel-sa-00615.html
///  - https://www.intel.com/content/www/us/en/developer/articles/technical/software-security-guidance/technical-documentation/processor-mmio-stale-data-vulnerabilities.html#inpage-nav-3-2-2
pub(crate) unsafe fn copy_to_userspace(src: *const u8, dst: *mut u8, len: usize) {
    /// 类似于 `ptr::copy(src, dst, len)`，区别在于它对非对齐写入使用 Intel 推荐的
    /// 指令序列。
    unsafe fn write_bytewise_to_userspace(src: *const u8, dst: *mut u8, len: usize) {
        if intrinsics::likely(len == 0) {
            return;
        }

        unsafe {
            let mut seg_sel: u16 = 0;
            for off in 0..len {
                asm!("
                    mov %ds, ({seg_sel})
                    verw ({seg_sel})
                    movb {val}, ({dst})
                    mfence
                    lfence
                    ",
                    val = in(reg_byte) *src.add(off),
                    dst = in(reg) dst.add(off),
                    seg_sel = in(reg) &mut seg_sel,
                    options(nostack, att_syntax)
                );
            }
        }
    }

    assert!(!src.is_null());
    assert!(!dst.is_null());
    assert!(is_enclave_range(src, len));
    assert!(is_user_range(dst, len));
    assert!(len < isize::MAX as usize);
    assert!(!src.addr().overflowing_add(len).1);
    assert!(!dst.addr().overflowing_add(len).1);

    unsafe {
        let (len1, len2, len3) = u64_align_to_guaranteed(dst, len);
        let (src1, dst1) = (src, dst);
        let (src2, dst2) = (src1.add(len1), dst1.add(len1));
        let (src3, dst3) = (src2.add(len2), dst2.add(len2));

        write_bytewise_to_userspace(src1, dst1, len1);
        copy_quadwords(src2, dst2, len2);
        write_bytewise_to_userspace(src3, dst3, len3);
    }
}

/// 把 `len` 字节的数据从用户空间指针 `src` 复制到 enclave 指针 `dst`
///
/// 此函数通过确保对不可信内存的所有读取都按 8 字节对齐，来缓解 AEPIC 泄漏漏洞
///
/// # Panics
/// 此函数在以下情况下 panic：
///
/// * `src` 指针为空
/// * `dst` 指针为空
/// * `src` 内存范围不在用户内存中
/// * `dst` 内存范围不在 enclave 内存中
///
/// # 参考资料(References)
///  - https://www.intel.com/content/www/us/en/security-center/advisory/intel-sa-00657.html
///  - https://www.intel.com/content/www/us/en/developer/articles/technical/software-security-guidance/advisory-guidance/stale-data-read-from-xapic.html
pub(crate) unsafe fn copy_from_userspace(src: *const u8, dst: *mut u8, len: usize) {
    /// 类似于 `ptr::copy(src, dst, len)`，区别在于它只使用按 u64 对齐的读取。
    ///
    /// # 安全性(Safety）
    /// 源内存区域不得跨越对齐边界。
    unsafe fn read_misaligned_from_userspace(src: *const u8, dst: *mut u8, len: usize) {
        if intrinsics::likely(len == 0) {
            return;
        }

        unsafe {
            let offset: usize;
            let data: u64;
            // 执行一次对 `src` 可能越界的内存读取，Rust 不支持这种操作，因此不得不
            // 使用汇编
            asm!("
                movl {src:e}, {offset:e}
                andl $7, {offset:e}
                andq $-8, {src}
                movq ({src}), {dst}
                ",
                src = inout(reg) src => _,
                offset = out(reg) offset,
                dst = out(reg) data,
                options(nostack, att_syntax, readonly, pure)
            );
            let data = data.to_le_bytes();
            ptr::copy_nonoverlapping(data.as_ptr().add(offset), dst, len);
        }
    }

    assert!(!src.is_null());
    assert!(!dst.is_null());
    assert!(is_user_range(src, len));
    assert!(is_enclave_range(dst, len));
    assert!(len < isize::MAX as usize);
    assert!(!(src as usize).overflowing_add(len).1);
    assert!(!(dst as usize).overflowing_add(len).1);

    unsafe {
        let (len1, len2, len3) = u64_align_to_guaranteed(src, len);
        let (src1, dst1) = (src, dst);
        let (src2, dst2) = (src1.add(len1), dst1.add(len1));
        let (src3, dst3) = (src2.add(len2), dst2.add(len2));

        read_misaligned_from_userspace(src1, dst1, len1);
        copy_quadwords(src2, dst2, len2);
        read_misaligned_from_userspace(src3, dst3, len3);
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<T: ?Sized> UserRef<T>
where
    T: UserSafe,
{
    /// 从一个裸指针创建一个 `&UserRef<[T]>`。
    ///
    /// # 安全性(Safety）
    /// 调用方必须确保 `ptr` 指向 `T`。
    ///
    /// # Panics
    /// 此函数在以下情况下 panic：
    ///
    /// * 指针未对齐
    /// * 指针为空
    /// * 所指向的范围不在用户内存中
    pub unsafe fn from_ptr<'a>(ptr: *const T) -> &'a Self {
        // SAFETY: 调用方必须遵守 `from_ptr` 的安全契约。
        unsafe { T::check_ptr(ptr) };
        unsafe { &*(ptr as *const Self) }
    }

    /// 从一个裸指针创建一个 `&mut UserRef<[T]>`。关于 `&mut UserRef<T>` 的微妙之处，
    /// 请参见该结构体的文档。
    ///
    /// # 安全性(Safety）
    /// 调用方必须确保 `ptr` 指向 `T`。
    ///
    /// # Panics
    /// 此函数在以下情况下 panic：
    ///
    /// * 指针未对齐
    /// * 指针为空
    /// * 所指向的范围不在用户内存中
    pub unsafe fn from_mut_ptr<'a>(ptr: *mut T) -> &'a mut Self {
        // SAFETY: 调用方必须遵守 `from_mut_ptr` 的安全契约。
        unsafe { T::check_ptr(ptr) };
        unsafe { &mut *(ptr as *mut Self) }
    }

    /// 把 `val` 复制到用户内存中。
    ///
    /// # Panics
    /// 如果目标与源的大小不同，则此函数 panic。对于切片等动态大小类型（DST），
    /// 这种情况可能发生。
    pub fn copy_from_enclave(&mut self, val: &T) {
        unsafe {
            assert_eq!(size_of_val(val), size_of_val(&*self.0.get()));
            copy_to_userspace(
                val as *const T as *const u8,
                self.0.get() as *mut T as *mut u8,
                size_of_val(val),
            );
        }
    }

    /// 从用户内存中复制该值，并将其放入 `dest`。
    ///
    /// # Panics
    /// 如果目标与源的大小不同，则此函数 panic。对于切片等动态大小类型（DST），
    /// 这种情况可能发生。
    pub fn copy_to_enclave<U: ?Sized + UserSafeCopyDestination<T>>(&self, dest: &mut U) {
        unsafe {
            assert_eq!(size_of_val(dest), size_of_val(&*self.0.get()));
            copy_from_userspace(
                self.0.get() as *const T as *const u8,
                dest.as_mut_ptr() as *mut u8,
                size_of_val(dest),
            );
        }
    }

    /// 从该引用获取一个裸指针。
    pub fn as_raw_ptr(&self) -> *const T {
        self as *const _ as _
    }

    /// 从该引用获取一个裸指针。
    pub fn as_raw_mut_ptr(&mut self) -> *mut T {
        self as *mut _ as _
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<T> UserRef<T>
where
    T: UserSafe,
{
    /// 从用户内存中把该值复制到 enclave 内存中。
    pub fn to_enclave(&self) -> T {
        unsafe {
            let mut data = mem::MaybeUninit::uninit();
            copy_from_userspace(self.0.get() as _, data.as_mut_ptr() as _, size_of::<T>());
            data.assume_init()
        }
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<T> UserRef<[T]>
where
    [T]: UserSafe,
{
    /// 从一个裸瘦指针（thin pointer）和切片长度创建一个 `&UserRef<[T]>`。
    ///
    /// # 安全性(Safety）
    /// 调用方必须确保 `ptr` 指向 `n` 个 `T` 类型的元素。
    ///
    /// # Panics
    /// 此函数在以下情况下 panic：
    ///
    /// * 指针未对齐
    /// * 指针为空
    /// * 所指向的范围无法放进地址空间
    /// * 所指向的范围不在用户内存中
    pub unsafe fn from_raw_parts<'a>(ptr: *const T, len: usize) -> &'a Self {
        // SAFETY: 调用方必须遵守 `from_raw_parts` 的安全契约。
        unsafe { &*(<[T]>::from_raw_sized(ptr as _, len * size_of::<T>()).as_ptr() as *const Self) }
    }

    /// 从一个裸瘦指针（thin pointer）和切片长度创建一个 `&mut UserRef<[T]>`。
    /// 关于 `&mut UserRef<T>` 的微妙之处，请参见该结构体的文档。
    ///
    /// # 安全性(Safety）
    /// 调用方必须确保 `ptr` 指向 `n` 个 `T` 类型的元素。
    ///
    /// # Panics
    /// 此函数在以下情况下 panic：
    ///
    /// * 指针未对齐
    /// * 指针为空
    /// * 所指向的范围无法放进地址空间
    /// * 所指向的范围不在用户内存中
    pub unsafe fn from_raw_parts_mut<'a>(ptr: *mut T, len: usize) -> &'a mut Self {
        // SAFETY: 调用方必须遵守 `from_raw_parts_mut` 的安全契约。
        unsafe {
            &mut *(<[T]>::from_raw_sized(ptr as _, len * size_of::<T>()).as_ptr() as *mut Self)
        }
    }

    /// 获取指向该用户切片首个元素的裸指针。
    pub fn as_ptr(&self) -> *const T {
        self.0.get() as _
    }

    /// 获取指向该用户切片首个元素的裸指针。
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.0.get() as _
    }

    /// 获取该用户切片中的元素数量。
    pub fn len(&self) -> usize {
        unsafe { self.0.get().len() }
    }

    /// 从用户内存中复制该值，并将其追加到 `dest`。
    pub fn append_to_enclave_vec(&self, dest: &mut Vec<T>) {
        dest.reserve(self.len());
        self.copy_to_enclave(&mut dest.spare_capacity_mut()[..self.len()]);
        // SAFETY: 我们在上面预留了足够的空间。
        unsafe { dest.set_len(dest.len() + self.len()) };
    }

    /// 从用户内存中把该值复制到 enclave 内存中的一个 vector 里。
    pub fn to_enclave(&self) -> Vec<T> {
        let mut ret = Vec::with_capacity(self.len());
        self.append_to_enclave_vec(&mut ret);
        ret
    }

    /// 返回对该切片的迭代器。
    pub fn iter(&self) -> Iter<'_, T>
    where
        T: UserSafe, // FIXME: should be implied by [T]: UserSafe?
    {
        unsafe { Iter((&*self.as_raw_ptr()).iter()) }
    }

    /// 返回一个允许修改每个值的迭代器。
    pub fn iter_mut(&mut self) -> IterMut<'_, T>
    where
        T: UserSafe, // FIXME: should be implied by [T]: UserSafe?
    {
        unsafe { IterMut((&mut *self.as_raw_mut_ptr()).iter_mut()) }
    }
}

/// 不可变用户切片迭代器
///
/// 此结构体由 `UserRef<[T]>` 上的 `iter` 方法创建。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub struct Iter<'a, T: 'a + UserSafe>(slice::Iter<'a, T>);

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<'a, T: UserSafe> Iterator for Iter<'a, T> {
    type Item = &'a UserRef<T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        unsafe { self.0.next().map(|e| UserRef::from_ptr(e)) }
    }
}

/// 可变用户切片迭代器
///
/// 此结构体由 `UserRef<[T]>` 上的 `iter_mut` 方法创建。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub struct IterMut<'a, T: 'a + UserSafe>(slice::IterMut<'a, T>);

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<'a, T: UserSafe> Iterator for IterMut<'a, T> {
    type Item = &'a mut UserRef<T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        unsafe { self.0.next().map(|e| UserRef::from_mut_ptr(e)) }
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<T: ?Sized> Deref for User<T>
where
    T: UserSafe,
{
    type Target = UserRef<T>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.as_ptr() }
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<T: ?Sized> DerefMut for User<T>
where
    T: UserSafe,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0.as_ptr() }
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<T: ?Sized> Drop for User<T>
where
    T: UserSafe,
{
    fn drop(&mut self) {
        unsafe {
            let ptr = (*self.0.as_ptr()).0.get();
            super::free(ptr as _, size_of_val(&mut *ptr), T::align_of());
        }
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<T: CoerceUnsized<U>, U> CoerceUnsized<UserRef<U>> for UserRef<T> {}

#[unstable(feature = "pin_coerce_unsized_trait", issue = "150112")]
unsafe impl<T: ?Sized> PinCoerceUnsized for UserRef<T> {}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<T, I> Index<I> for UserRef<[T]>
where
    [T]: UserSafe,
    I: SliceIndex<[T]>,
    I::Output: UserSafe,
{
    type Output = UserRef<I::Output>;

    #[inline]
    fn index(&self, index: I) -> &UserRef<I::Output> {
        unsafe {
            if let Some(slice) = index.get(&*self.as_raw_ptr()) {
                UserRef::from_ptr(slice)
            } else {
                rtabort!("index out of range for user slice");
            }
        }
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<T, I> IndexMut<I> for UserRef<[T]>
where
    [T]: UserSafe,
    I: SliceIndex<[T]>,
    I::Output: UserSafe,
{
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut UserRef<I::Output> {
        unsafe {
            if let Some(slice) = index.get_mut(&mut *self.as_raw_mut_ptr()) {
                UserRef::from_mut_ptr(slice)
            } else {
                rtabort!("index out of range for user slice");
            }
        }
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl UserRef<super::raw::ByteBuffer> {
    /// 把用户 `ByteBuffer` 所指向的用户内存范围复制到 enclave 内存中。
    ///
    /// # Panics
    /// 此函数在用户 `ByteBuffer` 中出现以下情况时 panic：
    ///
    /// * 指针为空
    /// * 所指向的范围无法放进地址空间
    /// * 所指向的范围不在用户内存中
    pub fn copy_user_buffer(&self) -> Vec<u8> {
        unsafe {
            let buf = self.to_enclave();
            if buf.len > 0 {
                User::from_raw_parts(buf.data as _, buf.len).to_enclave()
            } else {
                // 如果 `len` 为 `0`，绝不能查看 `data` 或调用 `free`。
                Vec::with_capacity(0)
            }
        }
    }
}
