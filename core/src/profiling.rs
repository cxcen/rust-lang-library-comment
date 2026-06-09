//! 供编译器插桩使用的 profiling 标记。

/// move 操作的 profiling 标记。
///
/// 该函数不会在运行时被调用。启用 `-Z annotate-moves` 时，编译器会生成合成调试信息，
/// 让 move 操作在 profiler 中表现为对该函数的调用。
///
/// `SIZE` 参数编码被复制类型的大小。它等同于 `size_of::<T>()`，
/// 只是为了 profiler/调试信息方便而存在。
#[unstable(feature = "profiling_marker_api", issue = "148197")]
#[lang = "compiler_move"]
pub fn compiler_move<T, const SIZE: usize>(_src: *const T, _dst: *mut T) {
    unreachable!(
        "compiler_move marks where the compiler-generated a memcpy for moves. It is never actually called."
    )
}

/// copy 操作的 profiling 标记。
///
/// 该函数不会在运行时被调用。启用 `-Z annotate-moves` 时，编译器会生成合成调试信息，
/// 让 copy 操作在 profiler 中表现为对该函数的调用。
///
/// `SIZE` 参数编码被复制类型的大小。它等同于 `size_of::<T>()`，
/// 只是为了 profiler/调试信息方便而存在。
#[unstable(feature = "profiling_marker_api", issue = "148197")]
#[lang = "compiler_copy"]
pub fn compiler_copy<T, const SIZE: usize>(_src: *const T, _dst: *mut T) {
    unreachable!(
        "compiler_copy marks where the compiler-generated a memcpy for Copies. It is never actually called."
    )
}
