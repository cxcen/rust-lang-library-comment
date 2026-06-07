//! 定义与 C 类型定义匹配的基本类型,用于 FFI 兼容。
//!
//! 本模块有意保持独立,以便提取 core C 类型时解析。

macro_rules! type_alias {
    {
      $Docfile:tt, $Alias:ident = $Real:ty;
      $( $Cfg:tt )*
    } => {
        #[doc = include_str!($Docfile)]
        $( $Cfg )*
        #[stable(feature = "core_ffi_c", since = "1.64.0")]
        pub type $Alias = $Real;
    }
}

type_alias! { "c_char.md", c_char = c_char_definition::c_char; #[doc(cfg(all()))] }

type_alias! { "c_schar.md", c_schar = i8; }
type_alias! { "c_uchar.md", c_uchar = u8; }
type_alias! { "c_short.md", c_short = i16; }
type_alias! { "c_ushort.md", c_ushort = u16; }

type_alias! { "c_int.md", c_int = c_int_definition::c_int; #[doc(cfg(all()))] }
type_alias! { "c_uint.md", c_uint = c_int_definition::c_uint; #[doc(cfg(all()))] }

type_alias! { "c_long.md", c_long = c_long_definition::c_long; #[doc(cfg(all()))] }
type_alias! { "c_ulong.md", c_ulong = c_long_definition::c_ulong; #[doc(cfg(all()))] }

type_alias! { "c_longlong.md", c_longlong = i64; }
type_alias! { "c_ulonglong.md", c_ulonglong = u64; }

type_alias! { "c_float.md", c_float = f32; }
type_alias! { "c_double.md", c_double = f64; }

mod c_char_definition {
    crate::cfg_select! {
        // 以下目标上的 c_char 是无符号的。通常同一架构上的所有 target_os 都拥有相同符号性,
        // 但也存在例外(见 clang 中的 isSignedCharDefault())。
        // aarch64:
        //   Procedure Call Standard for the Arm® 64-bit Architecture (AArch64)
        //   第 10 节 "Arm C and C++ language mappings" 说明 C/C++ char 是 unsigned byte。
        //   https://github.com/ARM-software/abi-aa/blob/2024Q3/aapcs64/aapcs64.rst#arm-c-and-c-language-mappings
        // arm:
        //   Procedure Call Standard for the Arm® Architecture 第 8 节
        //   "Arm C and C++ Language Mappings" 说明 C/C++ char 是 unsigned byte。
        //   https://github.com/ARM-software/abi-aa/blob/2024Q3/aapcs32/aapcs32.rst#arm-c-and-c-language-mappings
        // csky:
        //   C-SKY V2 CPU Applications Binary Interface Standards Manual 第 2.1.2 节
        //   "Primary Data Type" 说明 ANSI C char 是 unsigned byte。
        //   https://github.com/c-sky/csky-doc/blob/9f7121f7d40970ba5cc0f15716da033db2bb9d07/C-SKY_V2_CPU_Applications_Binary_Interface_Standards_Manual.pdf
        //   注意:这似乎与 Clang 默认值不匹配(https://github.com/rust-lang/rust/issues/129945)。
        // hexagon:
        //   Qualcomm Hexagon™ Application Binary Interface User Guide 第 3.1 节
        //   "Basic data type" 说明默认情况下 `char` 数据类型是 unsigned。
        //   https://docs.qualcomm.com/bundle/publicresource/80-N2040-23_REV_K_Qualcomm_Hexagon_Application_Binary_Interface_User_Guide.pdf
        // msp430:
        //   MSP430 Embedded Application Binary Interface 第 2.1 节 "Basic Types"
        //   说明 char 类型默认是 unsigned。
        //   https://www.ti.com/lit/an/slaa534a/slaa534a.pdf
        // powerpc/powerpc64:
        //   - PPC32 SysV: System V Application Binary Interface PowerPC Processor Supplement
        //     中的 "Table 3-1 Scalar Types" 说明 ANSI C char 是 unsigned byte
        //     https://refspecs.linuxfoundation.org/elf/elfspec_ppc.pdf
        //   - PPC64 ELFv1: 64-bit PowerPC ELF Application Binary Interface Supplement 1.9
        //     的 Section 3.1.4 "Fundamental Types" 说明 ANSI C char 是 unsigned byte
        //     https://refspecs.linuxfoundation.org/ELF/ppc64/PPC-elf64abi.html#FUND-TYPE
        //   - PPC64 ELFv2: 64-Bit ELF V2 ABI Specification 的 Section 2.1.2.2
        //     "Fundamental Types" 说明 char 是 unsigned byte
        //     https://openpowerfoundation.org/specifications/64bitelfabi/
        //   - AIX: XL C for AIX Language Reference 说明默认情况下 char 的行为类似 unsigned char。
        //     https://www.ibm.com/docs/en/xl-c-aix/13.1.3?topic=specifiers-character-types
        // riscv32/riscv64:
        //   RISC-V ELF psABI Document 的 RISC-V Calling Conventions 页中
        //   C/C++ type representations 小节说明 char 是 unsigned。
        //   https://github.com/riscv-non-isa/riscv-elf-psabi-doc/blob/draft-20240829-13bfa9f54634cb60d86b9b333e109f077805b4b3/riscv-cc.adoc#cc-type-representations
        // s390x:
        //   - ELF: ELF Application Binary Interface s390x Supplement Version 1.6.1 中的
        //     "Table 1.1.: Scalar types" 把 ISO C char 归类为 unsigned integer
        //     https://github.com/IBM/s390x-abi/releases/tag/v1.6.1
        //   - z/OS: XL C/C++ Language Reference 说明默认情况下 char 的行为类似 unsigned char。
        //     https://www.ibm.com/docs/en/zos/3.1.0?topic=specifiers-character-types
        // xtensa:
        //   Xtensa LX Microprocessor Overview handbook 第 2.17.1 节
        //   "Data Types and Alignment" 说明 `char` 类型默认是 unsigned。
        //   https://loboris.eu/ESP32/Xtensa_lx%20Overview%20handbook.pdf
        //
        // 在以下操作系统上,无论架构如何,c_char 默认都是有符号的。
        // Darwin(macOS、iOS 等):
        //   Apple 目标即使在 arm 上,c_char 默认也是有符号的
        //   https://developer.apple.com/documentation/xcode/writing-arm64-code-for-apple-platforms#Handle-data-types-and-data-alignment-properly
        // Windows:
        //   Windows MSVC C++ Language Reference 说明:除非使用 /J 编译选项,否则 char 类型变量
        //   默认会像 signed char 那样提升为 int。
        //   https://learn.microsoft.com/en-us/cpp/cpp/fundamental-types-cpp?view=msvc-170#character-types
        // Vita:
        //   Vita 上 char 默认有符号,VITASDK 遵循该约定。
        //   https://github.com/vitasdk/buildscripts/blob/09c533b771591ecde88864b6acad28ffb688dbd4/patches/gcc/0001-gcc-10.patch#L33-L34
        //
        // L4Re:
        //   内核在所有目标上都以 -funsigned-char 构建(但用户空间遵循架构默认值)。
        //   由于我们只有用户空间应用目标,所以下面不为 L4Re 设置特殊情况。
        //   https://github.com/rust-lang/rust/pull/132975#issuecomment-2484645240
        all(
            not(windows),
            not(target_vendor = "apple"),
            not(target_os = "vita"),
            any(
                target_arch = "aarch64",
                target_arch = "arm",
                target_arch = "csky",
                target_arch = "hexagon",
                target_arch = "msp430",
                target_arch = "powerpc",
                target_arch = "powerpc64",
                target_arch = "riscv32",
                target_arch = "riscv64",
                target_arch = "s390x",
                target_arch = "xtensa",
            )
        ) => {
            pub(super) type c_char = u8;
        }
        // 在所有其他目标上,c_char 是有符号的。
        _ => {
            pub(super) type c_char = i8;
        }
    }
}

mod c_long_definition {
    crate::cfg_select! {
        any(
            all(target_pointer_width = "64", not(windows)),
            // wasm32 Linux ABI 使用 64 位 long。
            all(target_arch = "wasm32", target_os = "linux")
        ) => {
            pub(super) type c_long = i64;
            pub(super) type c_ulong = u64;
        }
        _ => {
            // C 标准中 `long` 的最小大小为 32 位。
            pub(super) type c_long = i32;
            pub(super) type c_ulong = u32;
        }
    }
}

/// 等价于 C 的 `size_t` 类型,来自 `stddef.h`(或 C++ 的 `cstddef`)。
///
/// 本类型目前始终是 [`usize`],但未来可能存在不满足这一点的平台。
#[unstable(feature = "c_size_t", issue = "88345")]
pub type c_size_t = usize;

/// 等价于 C 的 `ptrdiff_t` 类型,来自 `stddef.h`(或 C++ 的 `cstddef`)。
///
/// 本类型目前始终是 [`isize`],但未来可能存在不满足这一点的平台。
#[unstable(feature = "c_size_t", issue = "88345")]
pub type c_ptrdiff_t = isize;

/// 等价于 C 的 `ssize_t`(POSIX 上)或 `SSIZE_T`(Windows 上)类型。
///
/// 本类型目前始终是 [`isize`],但未来可能存在不满足这一点的平台。
#[unstable(feature = "c_size_t", issue = "88345")]
pub type c_ssize_t = isize;

mod c_int_definition {
    crate::cfg_select! {
        any(target_arch = "avr", target_arch = "msp430") => {
            pub(super) type c_int = i16;
            pub(super) type c_uint = u16;
        }
        _ => {
            pub(super) type c_int = i32;
            pub(super) type c_uint = u32;
        }
    }
}
