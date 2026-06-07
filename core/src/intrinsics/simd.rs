//! SIMD（单指令多数据）编译器 intrinsic。
//!
//! 在本模块中，“向量”（vector）指任何带有 `repr(simd)` 的类型。
//!
//! 设计背景：这些 intrinsic 是编译器内建操作，是 core 与编译器（rustc/LLVM）之间的契约层。
//! 它们全部不稳定（unstable），普通用户不直接使用，而是由上层（如 `std::simd` / `core::simd`
//! 的可移植 SIMD 抽象）封装后暴露。许多 intrinsic 是 `unsafe`，带有严格前置条件（如下标越界、
//! mask 取值、除零等），一旦违反就是 UB；又因为它们是 intrinsic，编译器会据此做激进优化，
//! 误用的后果往往更隐蔽。

use crate::marker::ConstParamTy;

/// 向向量中插入一个元素，返回更新后的向量。
///
/// `T` 必须是元素类型为 `U` 的向量，且 `idx` 必须是 `const`（编译期常量）。
///
/// # 安全性（Safety）
///
/// `idx` 必须落在向量的下标范围内。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_insert<T, U>(x: T, idx: u32, val: U) -> T;

/// 从向量中取出一个元素。
///
/// `T` 必须是元素类型为 `U` 的向量，且 `idx` 必须是 `const`（编译期常量）。
///
/// # 安全性（Safety）
///
/// `idx` 必须是常量且落在向量的下标范围内。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_extract<T, U>(x: T, idx: u32) -> U;

/// 向向量中插入一个元素，返回更新后的向量。
///
/// `T` 必须是元素类型为 `U` 的向量。
///
/// 如果下标是 `const`，[`simd_insert`] 可能生成更优的汇编。
///
/// # 安全性（Safety）
///
/// `idx` 必须落在向量的下标范围内。
#[rustc_nounwind]
#[rustc_intrinsic]
pub unsafe fn simd_insert_dyn<T, U>(mut x: T, idx: u32, val: U) -> T {
    // SAFETY: `idx` 必须落在下标范围内（由调用方保证）。
    unsafe { (&raw mut x).cast::<U>().add(idx as usize).write(val) }
    x
}

/// 从向量中取出一个元素。
///
/// `T` 必须是元素类型为 `U` 的向量。
///
/// 如果下标是 `const`，[`simd_extract`] 可能生成更优的汇编。
///
/// # 安全性（Safety）
///
/// `idx` 必须落在向量的下标范围内。
#[rustc_nounwind]
#[rustc_intrinsic]
pub unsafe fn simd_extract_dyn<T, U>(x: T, idx: u32) -> U {
    // SAFETY: `idx` 必须落在下标范围内（由调用方保证）。
    unsafe { (&raw const x).cast::<U>().add(idx as usize).read() }
}

/// 逐元素地把两个 SIMD 向量相加。
///
/// `T` 必须是整数或浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_add<T>(x: T, y: T) -> T;

/// 逐元素地用 `lhs` 减去 `rhs`。
///
/// `T` 必须是整数或浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_sub<T>(lhs: T, rhs: T) -> T;

/// 逐元素地把两个 SIMD 向量相乘。
///
/// `T` 必须是整数或浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_mul<T>(x: T, y: T) -> T;

/// 逐元素地用 `lhs` 除以 `rhs`。
///
/// `T` 必须是整数或浮点数向量。
///
/// # 安全性（Safety）
/// 对整数而言，`rhs` 不得包含任何为零的元素。
/// 此外对有符号整数而言，`<int>::MIN / -1` 是未定义行为（UB）。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_div<T>(lhs: T, rhs: T) -> T;

/// 逐元素地返回两个向量相除的余数。
///
/// `T` 必须是整数或浮点数向量。
///
/// # 安全性（Safety）
/// 对整数而言，`rhs` 不得包含任何为零的元素。
/// 此外对有符号整数而言，`<int>::MIN / -1` 是未定义行为（UB）。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_rem<T>(lhs: T, rhs: T) -> T;

/// 逐元素地把向量左移，溢出时为 UB。
///
/// 把 `lhs` 左移 `rhs` 位；对有符号类型会移入符号位。
///
/// `T` 必须是整数向量。
///
/// # 安全性（Safety）
///
/// `rhs` 的每个元素都必须小于 `<int>::BITS`（对应整数类型的位宽）。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_shl<T>(lhs: T, rhs: T) -> T;

/// 逐元素地把向量右移，溢出时为 UB。
///
/// `T` 必须是整数向量。
///
/// 把 `lhs` 右移 `rhs` 位；对有符号类型会移入符号位。
///
/// # 安全性（Safety）
///
/// `rhs` 的每个元素都必须小于 `<int>::BITS`（对应整数类型的位宽）。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_shr<T>(lhs: T, rhs: T) -> T;

/// 逐元素地对向量做漏斗左移（funnel shift left），溢出时为 UB。
///
/// 逐元素地把 `a` 与 `b` 拼接（`a` 位于高位的那一半），得到一个长度相同、但每个元素位宽翻倍的向量。
/// 然后把该向量逐元素左移 `shift` 位、移入零，再取出每个元素的高位那一半。如果 `a` 与 `b` 相同，
/// 这就等价于一次逐元素的循环左移（rotate left）。
///
/// `T` 必须是整数向量。
///
/// # 安全性（Safety）
///
/// `shift` 的每个元素都必须小于 `<int>::BITS`（对应整数类型的位宽）。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_funnel_shl<T>(a: T, b: T, shift: T) -> T;

/// 逐元素地对向量做漏斗右移（funnel shift right），溢出时为 UB。
///
/// 逐元素地把 `a` 与 `b` 拼接（`a` 位于高位的那一半），得到一个长度相同、但每个元素位宽翻倍的向量。
/// 然后把该向量逐元素右移 `shift` 位、移入零，再取出每个元素的低位那一半。如果 `a` 与 `b` 相同，
/// 这就等价于一次逐元素的循环右移（rotate right）。
///
/// `T` 必须是整数向量。
///
/// # 安全性（Safety）
///
/// `shift` 的每个元素都必须小于 `<int>::BITS`（对应整数类型的位宽）。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_funnel_shr<T>(a: T, b: T, shift: T) -> T;

/// 逐元素地对向量做按位与（"And"）。
///
/// `T` 必须是整数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_and<T>(x: T, y: T) -> T;

/// 逐元素地对向量做按位或（"Or"）。
///
/// `T` 必须是整数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_or<T>(x: T, y: T) -> T;

/// 逐元素地对向量做按位异或（"Exclusive or"）。
///
/// `T` 必须是整数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_xor<T>(x: T, y: T) -> T;

/// 逐元素地对向量做数值类型转换。
///
/// `T` 与 `U` 必须都是整数或浮点数向量，且长度必须相同。
///
/// 浮点转整数时，结果会被截断（truncate）；结果若越界则导致 UB。
/// 整数转浮点时，结果会被舍入（round）。
/// 其他情况则截断或扩展该值，并对有符号整数保持符号。
///
/// # 安全性（Safety）
/// 从整数类型转换总是安全的。
/// 两个浮点类型之间互转也总是安全的。
///
/// 浮点转整数采用截断，遵循与 `to_int_unchecked` 相同的规则。具体而言，每个元素都必须：
/// * 不是 `NaN`
/// * 不是无穷大
/// * 在截掉小数部分后，可被目标返回类型表示
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_cast<T, U>(x: T) -> U;

/// 逐元素地对向量做数值类型转换。
///
/// `T` 与 `U` 必须都是整数或浮点数向量，且长度必须相同。
///
/// 与 `simd_cast` 类似，但对浮点转整数采用饱和（saturate）处理（`NaN` 变为 0）。
/// 这与常规的 `as` 行为一致，并且总是安全的。
///
/// 浮点转整数时，结果会被截断（truncate）。
/// 整数转浮点时，结果会被舍入（round）。
/// 其他情况则截断或扩展该值，并对有符号整数保持符号。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_as<T, U>(x: T) -> U;

/// 逐元素地对向量取负。
///
/// `T` 必须是整数或浮点数向量。
///
/// 对 `-<int>::Min` 而言，常规 Rust 会因溢出而 panic，但用本 intrinsic 不会是 UB。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_neg<T>(x: T) -> T;

/// 逐元素地返回向量的绝对值。
///
/// `T` 必须是浮点原始类型的向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_fabs<T>(x: T) -> T;

/// 逐元素地返回两个向量的较小值。
///
/// `T` 必须是浮点原始类型的向量。
///
/// 遵循 IEEE-754 的 `minNum` 语义。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_fmin<T>(x: T, y: T) -> T;

/// 逐元素地返回两个向量的较大值。
///
/// `T` 必须是浮点原始类型的向量。
///
/// 遵循 IEEE-754 的 `maxNum` 语义。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_fmax<T>(x: T, y: T) -> T;

/// 逐元素地测试两个向量是否相等。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是整数向量，且元素数量与元素大小都与 `T` 相同。
///
/// 假返回 `0`，真返回 `!0`（全 1）。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_eq<T, U>(x: T, y: T) -> U;

/// 逐元素地测试两个向量是否不相等。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是整数向量，且元素数量与元素大小都与 `T` 相同。
///
/// 假返回 `0`，真返回 `!0`（全 1）。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_ne<T, U>(x: T, y: T) -> U;

/// 逐元素地测试 `x` 是否小于 `y`。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是整数向量，且元素数量与元素大小都与 `T` 相同。
///
/// 假返回 `0`，真返回 `!0`（全 1）。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_lt<T, U>(x: T, y: T) -> U;

/// 逐元素地测试 `x` 是否小于或等于 `y`。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是整数向量，且元素数量与元素大小都与 `T` 相同。
///
/// 假返回 `0`，真返回 `!0`（全 1）。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_le<T, U>(x: T, y: T) -> U;

/// 逐元素地测试 `x` 是否大于 `y`。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是整数向量，且元素数量与元素大小都与 `T` 相同。
///
/// 假返回 `0`，真返回 `!0`（全 1）。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_gt<T, U>(x: T, y: T) -> U;

/// 逐元素地测试 `x` 是否大于或等于 `y`。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是整数向量，且元素数量与元素大小都与 `T` 相同。
///
/// 假返回 `0`，真返回 `!0`（全 1）。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_ge<T, U>(x: T, y: T) -> U;

/// 按编译期常量下标对两个向量做混洗（shuffle）。
///
/// `T` 必须是向量。
///
/// `U` 必须是一个 **const** 的 `u32` 向量。这意味着它要么引用一个具名常量，要么以内联 const 表达式
/// （`const { ... }`）的形式给出。
///
/// `V` 必须是一个向量，其元素类型与 `T` 相同、长度与 `U` 相同。
///
/// 返回一个新向量，其中第 `i` 个元素取自 `xy[idx[i]]`，这里 `xy` 是 `x` 与 `y` 的拼接。
/// 若 `idx[i]` 越出 `xy` 的下标范围，则是一个编译期错误。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_shuffle<T, U, V>(x: T, y: T, idx: U) -> V;

/// 读取一个指针向量（gather，聚集读取）。
///
/// `T` 必须是向量。
///
/// `U` 必须是指向 `T` 元素类型的指针向量，且长度与 `T` 相同。
///
/// `V` 必须是整数向量，长度与 `T` 相同（元素大小任意）。
///
/// 对 `ptr` 中的每个指针，如果 `mask` 中对应的值是 `!0`，就读取该指针；
/// 否则如果 `mask` 中对应的值是 `0`，就返回 `val` 中对应的值。
///
/// # 安全性（Safety）
/// `T` 中未被 mask 掩掉的那些值必须可读，如同用 `<ptr>::read` 读取一样（例如要对元素类型对齐）。
///
/// `mask` 只能包含 `0` 或 `!0` 这两种值。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_gather<T, U, V>(val: T, ptr: U, mask: V) -> T;

/// 写入一个指针向量（scatter，分散写入）。
///
/// `T` 必须是向量。
///
/// `U` 必须是指向 `T` 元素类型的指针向量，且长度与 `T` 相同。
///
/// `V` 必须是整数向量，长度与 `T` 相同（元素大小任意）。
///
/// 对 `ptr` 中的每个指针，如果 `mask` 中对应的值是 `!0`，就把 `val` 中对应的值写入该指针；
/// 否则如果 `mask` 中对应的值是 `0`，则什么也不做。
///
/// 这些写入按从左到右的顺序发生（当其中两次写入存在重叠时，这一点很关键）。
///
/// # 安全性（Safety）
/// `T` 中未被 mask 掩掉的那些值必须可写，如同用 `<ptr>::write` 写入一样（例如要对元素类型对齐）。
///
/// `mask` 只能包含 `0` 或 `!0` 这两种值。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_scatter<T, U, V>(val: T, ptr: U, mask: V);

/// 用于 SIMD 掩码加载/存储 intrinsic 的对齐选项类型。
#[derive(Debug, ConstParamTy, PartialEq, Eq)]
pub enum SimdAlign {
    // 这些取值必须与编译器中 `rustc_middle/src/ty/consts/int.rs` 里定义的 `SimdAlign` 保持一致！
    /// 对指针没有对齐要求。
    Unaligned = 0,
    /// 指针必须对齐到 SIMD 向量的元素类型。
    Element = 1,
    /// 指针必须对齐到 SIMD 向量类型本身。
    Vector = 2,
}

/// 读取一个指针向量（带掩码的连续加载）。
///
/// `T` 必须是向量。
///
/// `U` 必须是指向 `T` 元素类型的指针。
///
/// `V` 必须是整数向量，长度与 `T` 相同（元素大小任意）。
///
/// 对每个元素，如果 `mask` 中对应的值是 `!0`，就从 `ptr` 的对应偏移处读取。
/// 第一个元素从 `ptr` 加载，第二个从 `ptr.wrapping_offset(1)` 加载，依此类推。
/// 否则如果 `mask` 中对应的值是 `0`，就返回 `val` 中对应的值。
///
/// # 安全性（Safety）
/// `ptr` 必须按 `ALIGN` 参数所要求的方式对齐，详见 [`SimdAlign`]。
///
/// `mask` 只能包含 `0` 或 `!0` 这两种值。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_masked_load<V, U, T, const ALIGN: SimdAlign>(mask: V, ptr: U, val: T)
-> T;

/// 写入一个指针向量（带掩码的连续存储）。
///
/// `T` 必须是向量。
///
/// `U` 必须是指向 `T` 元素类型的指针。
///
/// `V` 必须是整数向量，长度与 `T` 相同（元素大小任意）。
///
/// 对每个元素，如果 `mask` 中对应的值是 `!0`，就把 `val` 中对应的值写到 `ptr` 的对应偏移处。
/// 第一个元素写到 `ptr`，第二个写到 `ptr.wrapping_offset(1)`，依此类推。
/// 否则如果 `mask` 中对应的值是 `0`，则什么也不做。
///
/// # 安全性（Safety）
/// `ptr` 必须按 `ALIGN` 参数所要求的方式对齐，详见 [`SimdAlign`]。
///
/// `mask` 只能包含 `0` 或 `!0` 这两种值。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_masked_store<V, U, T, const ALIGN: SimdAlign>(mask: V, ptr: U, val: T);

/// 逐元素地把两个 SIMD 向量相加，带饱和（saturation）。
///
/// `T` 必须是整数原始类型的向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_saturating_add<T>(x: T, y: T) -> T;

/// 逐元素地把两个 SIMD 向量相减，带饱和（saturation）。
///
/// `T` 必须是整数原始类型的向量。
///
/// 用 `lhs` 减去 `rhs`。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_saturating_sub<T>(lhs: T, rhs: T) -> T;

/// 从左到右地把向量内的各元素相加（归约求和，有序）。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是 `T` 的元素类型。
///
/// 以初值 `y` 开始，依次加上 `x` 的各元素并累加。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_reduce_add_ordered<T, U>(x: T, y: U) -> U;

/// 以任意顺序把向量内的各元素相加。也可能在输入/输出上做无序的重结合（re-association）。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是 `T` 的元素类型。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn simd_reduce_add_unordered<T, U>(x: T) -> U;

/// 从左到右地把向量内的各元素相乘（归约求积，有序）。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是 `T` 的元素类型。
///
/// 以初值 `y` 开始，依次乘以 `x` 的各元素并累乘。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_reduce_mul_ordered<T, U>(x: T, y: U) -> U;

/// 以任意顺序把向量内的各元素相乘。也可能在输入/输出上做无序的重结合（re-association）。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是 `T` 的元素类型。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn simd_reduce_mul_unordered<T, U>(x: T) -> U;

/// 检查所有掩码值是否都为真。
///
/// `T` 必须是整数原始类型的向量。
///
/// # 安全性（Safety）
/// `x` 只能包含 `0` 或 `!0`。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_reduce_all<T>(x: T) -> bool;

/// 检查是否存在任一掩码值为真。
///
/// `T` 必须是整数原始类型的向量。
///
/// # 安全性（Safety）
/// `x` 只能包含 `0` 或 `!0`。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_reduce_any<T>(x: T) -> bool;

/// 返回向量中的最大元素。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是 `T` 的元素类型。
///
/// 对浮点值，使用 IEEE-754 的 `maxNum`。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_reduce_max<T, U>(x: T) -> U;

/// 返回向量中的最小元素。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是 `T` 的元素类型。
///
/// 对浮点值，使用 IEEE-754 的 `minNum`。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_reduce_min<T, U>(x: T) -> U;

/// 把所有元素做逻辑按位与（"and"）归约到一起。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是 `T` 的元素类型。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_reduce_and<T, U>(x: T) -> U;

/// 把所有元素做逻辑按位或（"or"）归约到一起。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是 `T` 的元素类型。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_reduce_or<T, U>(x: T) -> U;

/// 把所有元素做逻辑按位异或（"exclusive or"）归约到一起。
///
/// `T` 必须是整数或浮点数向量。
///
/// `U` 必须是 `T` 的元素类型。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_reduce_xor<T, U>(x: T) -> U;

/// 把整数向量截断成一个位掩码（bitmask）。
///
/// `T` 必须是整数向量。
///
/// `U` 必须是“位数至少与 `T` 长度相等的最小无符号整数”，或“位数至少与 `T` 长度相等的最小 `u8` 数组”。
///
/// 每个元素被截断成单个比特并打包进结果。
///
/// 无论输出是数组还是无符号整数，它都被当作一串连续的比特来看待。位掩码总是打包在输出的最低有效位一侧，
/// 高位用 0 填充。比特的顺序取决于字节序（endianness）：
///
/// * 在小端序上，最低有效位对应第一个向量元素。
/// * 在大端序上，最低有效位对应最后一个向量元素。
///
/// 例如，`[!0, 0, !0, !0]` 打包成
/// - 小端序：`0b1101u8` 或 `[0b1101]`；
/// - 大端序：`0b1011u8` 或 `[0b1011]`。
///
/// 再看一个更大的例子，
/// `[!0, 0, 0, 0, 0, 0, 0, 0, !0, !0, 0, 0, 0, 0, !0, 0]` 打包成
/// - 小端序：`0b0100001100000001u16` 或 `[0b00000001, 0b01000011]`；
/// - 大端序：`0b1000000011000010u16` 或 `[0b10000000, 0b11000010]`。
///
/// 最后看一个长度非 2 的幂、且跨多个字节的例子：
/// `[!0, !0, 0, !0, 0, 0, !0, 0, !0, 0]` 打包成
/// - 小端序：`0b0101001011u16` 或 `[0b01001011, 0b01]`；
/// - 大端序：`0b1101001010u16` 或 `[0b11, 0b01001010]`。
///
/// # 安全性（Safety）
/// `x` 只能包含 `0` 和 `!0`。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_bitmask<T, U>(x: T) -> U;

/// 按掩码选择元素。
///
/// `T` 必须是向量。
///
/// `M` 必须是整数向量，长度与 `T` 相同（元素大小任意）。
///
/// 对每个元素，如果 `mask` 中对应的值是 `!0`，就从 `if_true` 选取该元素；
/// 如果 `mask` 中对应的值是 `0`，就从 `if_false` 选取该元素。
///
/// # 安全性（Safety）
/// `mask` 只能包含 `0` 和 `!0`。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_select<M, T>(mask: M, if_true: T, if_false: T) -> T;

/// 按位掩码（bitmask）选择元素。
///
/// `M` 必须是无符号整数或 `u8` 数组，与 `simd_bitmask` 相匹配。
///
/// `T` 必须是向量。
///
/// 对每个元素，如果 `mask` 中对应的比特是 `1`，就从 `if_true` 选取该元素；
/// 如果对应的比特是 `0`，就从 `if_false` 选取该元素。
/// 掩码中其余的比特会被忽略。
///
/// 位掩码的比特顺序与 `simd_bitmask` 一致。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_select_bitmask<M, T>(m: M, yes: T, no: T) -> T;

/// 逐元素地从指针向量计算偏移，可能发生回绕（wrapping）。
///
/// `T` 必须是指针向量。
///
/// `U` 必须是 `isize` 或 `usize` 向量，元素数量与 `T` 相同。
///
/// 其行为如同 `<ptr>::wrapping_offset`。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_arith_offset<T, U>(ptr: T, offset: U) -> T;

/// 转换一个指针向量。
///
/// `T` 与 `U` 必须都是指针向量，且元素数量相同。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_cast_ptr<T, U>(ptr: T) -> U;

/// 把一个指针向量暴露（expose）为一个地址向量。
///
/// `T` 必须是指针向量。
///
/// `U` 必须是 `usize` 向量，长度与 `T` 相同。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn simd_expose_provenance<T, U>(ptr: T) -> U;

/// 从一个地址向量创建一个指针向量。
///
/// `T` 必须是 `usize` 向量。
///
/// `U` 必须是指针向量，长度与 `T` 相同。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_with_exposed_provenance<T, U>(addr: T) -> U;

/// 逐元素地交换每个元素的字节序（byte swap）。
///
/// `T` 必须是整数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_bswap<T>(x: T) -> T;

/// 逐元素地反转每个元素的比特（bit reverse）。
///
/// `T` 必须是整数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_bitreverse<T>(x: T) -> T;

/// 逐元素地统计每个元素的前导零个数（count leading zeros）。
///
/// `T` 必须是整数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_ctlz<T>(x: T) -> T;

/// 逐元素地统计每个元素中 1 的个数（population count）。
///
/// `T` 必须是整数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_ctpop<T>(x: T) -> T;

/// 逐元素地统计每个元素的尾随零个数（count trailing zeros）。
///
/// `T` 必须是整数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_cttz<T>(x: T) -> T;

/// 逐元素地向上取整，得到紧邻的、不小于原值的整数值浮点数（向上取整，ceil）。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_ceil<T>(x: T) -> T;

/// 逐元素地向下取整，得到紧邻的、不大于原值的整数值浮点数（向下取整，floor）。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_floor<T>(x: T) -> T;

/// 逐元素地把每个元素舍入到最接近的整数值浮点数。
/// 平局（恰好在两个整数中间）时，向远离 0 的方向舍入。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_round<T>(x: T) -> T;

/// 逐元素地把每个元素舍入到最接近的整数值浮点数。
/// 平局时，舍入到最低有效位为偶数的那个数（即“四舍六入五成双”）。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_round_ties_even<T>(x: T) -> T;

/// 逐元素地返回每个元素的整数部分，结果为整数值浮点数。
/// 换言之，非整数值会被向零截断（truncate towards zero）。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_trunc<T>(x: T) -> T;

/// 逐元素地对每个元素求平方根。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn simd_fsqrt<T>(x: T) -> T;

/// 逐元素地计算 `(x*y) + z`，且不做任何中间舍入（即融合乘加，FMA）。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_fma<T>(x: T, y: T, z: T) -> T;

/// 逐元素地计算 `(x*y) + z`，但以非确定性的方式，要么执行融合乘加（fused multiply-add），
/// 要么执行两步运算并对中间结果做舍入。
///
/// 当代码生成器判定目标指令集支持融合操作、且融合操作比等价的“分开的乘法 + 加法”两条指令更高效时，
/// 才会做融合。是否选择融合操作并未被规定，且可能取决于优化级别、上下文等因素。
/// 甚至可能出现某些 SIMD 通道（lane）做了融合而另一些没有的情况。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const unsafe fn simd_relaxed_fma<T>(x: T, y: T, z: T) -> T;

// 逐元素地计算每个元素的正弦（sine）。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn simd_fsin<T>(a: T) -> T;

// 逐元素地计算每个元素的余弦（cosine）。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn simd_fcos<T>(a: T) -> T;

// 逐元素地计算每个元素的指数函数（e 的幂，exp）。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn simd_fexp<T>(a: T) -> T;

// 逐元素地计算 2 的（每个元素次）幂。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn simd_fexp2<T>(a: T) -> T;

// 逐元素地计算每个元素以 10 为底的对数（log10）。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn simd_flog10<T>(a: T) -> T;

// 逐元素地计算每个元素以 2 为底的对数（log2）。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn simd_flog2<T>(a: T) -> T;

// 逐元素地计算每个元素的自然对数（以 e 为底，ln）。
///
/// `T` 必须是浮点数向量。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn simd_flog<T>(a: T) -> T;
