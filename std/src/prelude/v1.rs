//! Rust 标准库预导入的第一个版本。
//!
//! 更多内容参见[模块级文档](super)。

#![stable(feature = "rust1", since = "1.0.0")]

// 不做格式化：本文件除了重导出别无他物，而其顺序值得保留。
#![cfg_attr(rustfmt, rustfmt::skip)]

// 重导出的 core 运算符（operators）
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(no_inline)]
pub use crate::marker::{Send, Sized, Sync, Unpin};
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(no_inline)]
pub use crate::ops::{Drop, Fn, FnMut, FnOnce};
#[stable(feature = "async_closure", since = "1.85.0")]
#[doc(no_inline)]
pub use crate::ops::{AsyncFn, AsyncFnMut, AsyncFnOnce};

// 重导出的函数
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(no_inline)]
pub use crate::mem::drop;
#[stable(feature = "size_of_prelude", since = "1.80.0")]
#[doc(no_inline)]
pub use crate::mem::{align_of, align_of_val, size_of, size_of_val};

// 重导出的类型与 trait
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(no_inline)]
pub use crate::convert::{AsMut, AsRef, From, Into};
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(no_inline)]
pub use crate::iter::{DoubleEndedIterator, ExactSizeIterator};
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(no_inline)]
pub use crate::iter::{Extend, IntoIterator, Iterator};
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(no_inline)]
pub use crate::option::Option::{self, None, Some};
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(no_inline)]
pub use crate::result::Result::{self, Err, Ok};

// 重导出的内建宏与 trait
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[doc(no_inline)]
#[expect(deprecated)]
pub use core::prelude::v1::{
    assert, assert_eq, assert_ne, cfg, column, compile_error, concat, debug_assert, debug_assert_eq,
    debug_assert_ne, env, file, format_args, include, include_bytes, include_str, line, matches,
    module_path, option_env, stringify, todo, r#try, unimplemented, unreachable, write,
    writeln, Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd,
};

#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[doc(no_inline)]
pub use crate::{
    dbg, eprint, eprintln, format, is_x86_feature_detected, print, println, thread_local
};

// 这些宏需要特殊处理，以免我们既导出它们*又*导出同名的模块。我们只想把这些宏放进
// 预导入，因此用同名的私有模块来遮蔽（shadow）原来的模块。
mod ambiguous_macros_only {
    #[expect(hidden_glob_reexports)]
    mod vec {}
    #[expect(hidden_glob_reexports)]
    mod panic {}
    // 在构建 std 时若不带 expect exported_private_dependencies，会产生警告；但那样
    // clippy 又会声称这是一个 useless_attribute。因此把两者都消音。
    #[expect(clippy::useless_attribute)]
    #[expect(exported_private_dependencies)]
    #[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
    pub use crate::*;
}
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[doc(no_inline)]
pub use self::ambiguous_macros_only::{vec, panic};

#[unstable(feature = "cfg_select", issue = "115585")]
#[doc(no_inline)]
pub use core::prelude::v1::cfg_select;

#[unstable(
    feature = "concat_bytes",
    issue = "87555",
    reason = "`concat_bytes` is not stable enough for use and is subject to change"
)]
#[doc(no_inline)]
pub use core::prelude::v1::concat_bytes;

#[unstable(feature = "const_format_args", issue = "none")]
#[doc(no_inline)]
pub use core::prelude::v1::const_format_args;

#[unstable(
    feature = "log_syntax",
    issue = "29598",
    reason = "`log_syntax!` is not stable enough for use and is subject to change"
)]
#[doc(no_inline)]
pub use core::prelude::v1::log_syntax;

#[unstable(
    feature = "trace_macros",
    issue = "29598",
    reason = "`trace_macros` is not stable enough for use and is subject to change"
)]
#[doc(no_inline)]
pub use core::prelude::v1::trace_macros;

// 不要 `doc(no_inline)`，以便它们各自成为独立的文档项
//（因为没有一个公开模块可供它们被重导出）。
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
pub use core::prelude::v1::{
    alloc_error_handler, bench, derive, global_allocator, test, test_case,
};

#[unstable(feature = "derive_const", issue = "118304")]
pub use core::prelude::v1::derive_const;

// 同样不要 `doc(no_inline)`。
#[unstable(
    feature = "cfg_accessible",
    issue = "64797",
    reason = "`cfg_accessible` is not fully implemented"
)]
pub use core::prelude::v1::cfg_accessible;

// 同样不要 `doc(no_inline)`。
#[unstable(
    feature = "cfg_eval",
    issue = "82679",
    reason = "`cfg_eval` is a recently implemented feature"
)]
pub use core::prelude::v1::cfg_eval;

// 同样不要 `doc(no_inline)`。
#[unstable(
    feature = "type_ascription",
    issue = "23416",
    reason = "placeholder syntax for type ascription"
)]
pub use core::prelude::v1::type_ascribe;

// 同样不要 `doc(no_inline)`。
#[unstable(
    feature = "deref_patterns",
    issue = "87121",
    reason = "placeholder syntax for deref patterns"
)]
pub use core::prelude::v1::deref;

// 同样不要 `doc(no_inline)`。
#[unstable(
    feature = "type_alias_impl_trait",
    issue = "63063",
    reason = "`type_alias_impl_trait` has open design concerns"
)]
pub use core::prelude::v1::define_opaque;

#[unstable(feature = "extern_item_impls", issue = "125418")]
pub use core::prelude::v1::{eii, unsafe_eii};

#[unstable(feature = "eii_internals", issue = "none")]
pub use core::prelude::v1::eii_declaration;

// 到目前为止，本文件等价于 core/src/prelude/v1.rs。这里采用重复定义而非 glob 导入，
// 是因为我们希望文档把这些重导出显示为指向 `std` 内部。
// 下面是来自 alloc crate 的各项。

#[stable(feature = "rust1", since = "1.0.0")]
#[doc(no_inline)]
pub use crate::borrow::ToOwned;
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(no_inline)]
pub use crate::boxed::Box;
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(no_inline)]
pub use crate::string::{String, ToString};
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(no_inline)]
pub use crate::vec::Vec;
