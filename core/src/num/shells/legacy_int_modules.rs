#![doc(hidden)]

macro_rules! legacy_int_module {
    ($T:ident) => (legacy_int_module!($T, #[stable(feature = "rust1", since = "1.0.0")]););
    ($T:ident, #[$attr:meta]) => (
        #[$attr]
        #[deprecated(
            since = "TBD",
            note = "all constants in this module replaced by associated constants on the type"
        )]
        #[rustc_diagnostic_item = concat!(stringify!($T), "_legacy_mod")]
        pub mod $T {
            #![doc = concat!("用于 [`", stringify!($T), "` 原语类型][", stringify!($T), "] 的冗余常量模块。")]
            //!
            //! 新代码应直接使用原语类型上的关联常量。

            #[doc = concat!(
                "此整数类型可表示的最小值。请改用 ",
                "[`", stringify!($T), "::MIN", "`]。"
            )]
            ///
            /// # 示例
            ///
            /// ```rust
            /// // 已弃用的写法
            #[doc = concat!("let min = std::", stringify!($T), "::MIN;")]
            ///
            /// // 推荐的写法
            #[doc = concat!("let min = ", stringify!($T), "::MIN;")]
            /// ```
            ///
            #[$attr]
            #[deprecated(since = "TBD", note = "replaced by the `MIN` associated constant on this type")]
            #[rustc_diagnostic_item = concat!(stringify!($T), "_legacy_const_min")]
            pub const MIN: $T = $T::MIN;

            #[doc = concat!(
                "此整数类型可表示的最大值。请改用 ",
                "[`", stringify!($T), "::MAX", "`]。"
            )]
            ///
            /// # 示例
            ///
            /// ```rust
            /// // 已弃用的写法
            #[doc = concat!("let max = std::", stringify!($T), "::MAX;")]
            ///
            /// // 推荐的写法
            #[doc = concat!("let max = ", stringify!($T), "::MAX;")]
            /// ```
            ///
            #[$attr]
            #[deprecated(since = "TBD", note = "replaced by the `MAX` associated constant on this type")]
            #[rustc_diagnostic_item = concat!(stringify!($T), "_legacy_const_max")]
            pub const MAX: $T = $T::MAX;
        }
    )
}

legacy_int_module! { i128, #[stable(feature = "i128", since = "1.26.0")] }
legacy_int_module! { i16 }
legacy_int_module! { i32 }
legacy_int_module! { i64 }
legacy_int_module! { i8 }
legacy_int_module! { isize }
legacy_int_module! { u128, #[stable(feature = "i128", since = "1.26.0")] }
legacy_int_module! { u16 }
legacy_int_module! { u32 }
legacy_int_module! { u64 }
legacy_int_module! { u8 }
legacy_int_module! { usize }
