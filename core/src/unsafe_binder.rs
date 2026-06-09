//! 用于把类型转换成 unsafe binder 以及转回来的操作符。

/// 将 unsafe binder 解包成其底层类型。
#[allow_internal_unstable(builtin_syntax)]
#[unstable(feature = "unsafe_binders", issue = "130516")]
pub macro unwrap_binder {
    ($expr:expr) => {
        builtin # unwrap_binder ( $expr )
    },
    ($expr:expr ; $ty:ty) => {
        builtin # unwrap_binder ( $expr, $ty )
    },
}

/// 将类型包装成 unsafe binder。
#[allow_internal_unstable(builtin_syntax)]
#[unstable(feature = "unsafe_binders", issue = "130516")]
pub macro wrap_binder {
    ($expr:expr) => {
        builtin # wrap_binder ( $expr )
    },
    ($expr:expr ; $ty:ty) => {
        builtin # wrap_binder ( $expr, $ty )
    },
}
