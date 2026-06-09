// 文档见 core/src/primitive_docs.rs。

use crate::cell::CloneFromCell;
use crate::cmp::Ordering::{self, *};
use crate::marker::{ConstParamTy_, StructuralPartialEq};
use crate::ops::ControlFlow::{self, Break, Continue};

// 递归宏，用于为 n 元组实现函数和操作。
//
// 也会为更低 arity 的元组提供实现。例如，tuple_impls!(A B C)
// 会为 (A, B, C)、(A, B) 和 (A,) 实现所有内容。
macro_rules! tuple_impls {
    // 停止条件（1 元组）
    ($T:ident) => {
        tuple_impls!(@impl $T);
    };
    // 递归条件（n 元组，n >= 2）
    ($T:ident $( $U:ident )+) => {
        tuple_impls!($( $U )+);
        tuple_impls!(@impl $T $( $U )+);
    };
    // “私有”内部实现
    (@impl $( $T:ident )+) => {
        maybe_tuple_doc! {
            $($T)+ @
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
            impl<$($T: [const] PartialEq),+> const PartialEq for ($($T,)+) {
                #[inline]
                fn eq(&self, other: &($($T,)+)) -> bool {
                    $( ${ignore($T)} self.${index()} == other.${index()} )&&+
                }
                #[inline]
                fn ne(&self, other: &($($T,)+)) -> bool {
                    $( ${ignore($T)} self.${index()} != other.${index()} )||+
                }
            }
        }

        maybe_tuple_doc! {
            $($T)+ @
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
            impl<$($T: [const] Eq),+> const Eq for ($($T,)+)
            {}
        }

        maybe_tuple_doc! {
            $($T)+ @
            #[unstable(feature = "adt_const_params", issue = "95174")]
            #[unstable_feature_bound(unsized_const_params)]
            impl<$($T: ConstParamTy_),+> ConstParamTy_ for ($($T,)+)
            {}
        }

        maybe_tuple_doc! {
            $($T)+ @
            #[unstable(feature = "structural_match", issue = "31434")]
            impl<$($T),+> StructuralPartialEq for ($($T,)+)
            {}
        }

        maybe_tuple_doc! {
            $($T)+ @
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
            impl<$($T: [const] PartialOrd),+> const PartialOrd for ($($T,)+)
            {
                #[inline]
                fn partial_cmp(&self, other: &($($T,)+)) -> Option<Ordering> {
                    lexical_partial_cmp!($( ${ignore($T)} self.${index()}, other.${index()} ),+)
                }
                #[inline]
                fn lt(&self, other: &($($T,)+)) -> bool {
                    lexical_ord!(lt, __chaining_lt, $( ${ignore($T)} self.${index()}, other.${index()} ),+)
                }
                #[inline]
                fn le(&self, other: &($($T,)+)) -> bool {
                    lexical_ord!(le, __chaining_le, $( ${ignore($T)} self.${index()}, other.${index()} ),+)
                }
                #[inline]
                fn ge(&self, other: &($($T,)+)) -> bool {
                    lexical_ord!(ge, __chaining_ge, $( ${ignore($T)} self.${index()}, other.${index()} ),+)
                }
                #[inline]
                fn gt(&self, other: &($($T,)+)) -> bool {
                    lexical_ord!(gt, __chaining_gt, $( ${ignore($T)} self.${index()}, other.${index()} ),+)
                }
                #[inline]
                fn __chaining_lt(&self, other: &($($T,)+)) -> ControlFlow<bool> {
                    lexical_chain!(__chaining_lt, $( ${ignore($T)} self.${index()}, other.${index()} ),+)
                }
                #[inline]
                fn __chaining_le(&self, other: &($($T,)+)) -> ControlFlow<bool> {
                    lexical_chain!(__chaining_le, $( ${ignore($T)} self.${index()}, other.${index()} ),+)
                }
                #[inline]
                fn __chaining_gt(&self, other: &($($T,)+)) -> ControlFlow<bool> {
                    lexical_chain!(__chaining_gt, $( ${ignore($T)} self.${index()}, other.${index()} ),+)
                }
                #[inline]
                fn __chaining_ge(&self, other: &($($T,)+)) -> ControlFlow<bool> {
                    lexical_chain!(__chaining_ge, $( ${ignore($T)} self.${index()}, other.${index()} ),+)
                }
            }
        }

        maybe_tuple_doc! {
            $($T)+ @
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
            impl<$($T: [const] Ord),+> const Ord for ($($T,)+)
            {
                #[inline]
                fn cmp(&self, other: &($($T,)+)) -> Ordering {
                    lexical_cmp!($( ${ignore($T)} self.${index()}, other.${index()} ),+)
                }
            }
        }

        maybe_tuple_doc! {
            $($T)+ @
            #[stable(feature = "rust1", since = "1.0.0")]
            impl<$($T: Default),+> Default for ($($T,)+) {
                #[inline]
                fn default() -> ($($T,)+) {
                    ($({ let x: $T = Default::default(); x},)+)
                }
            }
        }

        maybe_tuple_doc! {
            $($T)+ @
            #[stable(feature = "array_tuple_conv", since = "1.71.0")]
            // 由于 https://github.com/rust-lang/rust/issues/144280，目前不能实现 const From。
            impl<T> From<[T; ${count($T)}]> for ($(${ignore($T)} T,)+) {
                #[inline]
                #[allow(non_snake_case)]
                fn from(array: [T; ${count($T)}]) -> Self {
                    let [$($T,)+] = array;
                    ($($T,)+)
                }
            }
        }

        maybe_tuple_doc! {
            $($T)+ @
            #[stable(feature = "array_tuple_conv", since = "1.71.0")]
            // 由于 https://github.com/rust-lang/rust/issues/144280，目前不能实现 const From。
            impl<T> From<($(${ignore($T)} T,)+)> for [T; ${count($T)}] {
                #[inline]
                #[allow(non_snake_case)]
                fn from(tuple: ($(${ignore($T)} T,)+)) -> Self {
                    let ($($T,)+) = tuple;
                    [$($T,)+]
                }
            }
        }

        maybe_tuple_doc! {
            $($T)+ @
            // SAFETY: 元组不会引入额外间接层，所以只要 T 可复制，元组也可复制。
            #[unstable(feature = "cell_get_cloned", issue = "145329")]
            unsafe impl<$($T: CloneFromCell),+> CloneFromCell for ($($T,)+)
            {}
        }
    }
}

// 如果这是 1 元组，则添加文档注释。
// 否则完全隐藏文档。
macro_rules! maybe_tuple_doc {
    ($a:ident @ #[$meta:meta] $item:item) => {
        #[doc(fake_variadic)]
        #[doc = "该 trait 为长度最多十二项的元组实现。"]
        #[$meta]
        $item
    };
    ($a:ident $($rest_a:ident)+ @ #[$meta:meta] $item:item) => {
        #[doc(hidden)]
        #[$meta]
        $item
    };
}

// 构造一个表达式，使用方法 `$rel` 执行词典序比较。
// 值按交错顺序排列，因此针对
// `(a1, a2, a3) < (b1, b2, b3)` 的宏调用会是 `lexical_ord!(lt, opt_is_lt, a1, b1,
// a2, b2, a3, b3)`（`lexical_cmp` 同理）。
//
// `$chain_rel` 是来自 `PartialOrd` 的链式方法，会用于除最后一个值以外的所有值，
// 以便为简单 primitive 产生更好的结果。
macro_rules! lexical_ord {
    ($rel: ident, $chain_rel: ident, $a:expr, $b:expr, $($rest_a:expr, $rest_b:expr),+) => {{
        match PartialOrd::$chain_rel(&$a, &$b) {
            Break(val) => val,
            Continue(()) => lexical_ord!($rel, $chain_rel, $($rest_a, $rest_b),+),
        }
    }};
    ($rel: ident, $chain_rel: ident, $a:expr, $b:expr) => {
        // 对最后一个元素使用具体方法
        PartialOrd::$rel(&$a, &$b)
    };
}

// 参数交错方式同上方 `lexical_ord`。
macro_rules! lexical_chain {
    ($chain_rel: ident, $a:expr, $b:expr $(,$rest_a:expr, $rest_b:expr)*) => {{
        PartialOrd::$chain_rel(&$a, &$b)?;
        lexical_chain!($chain_rel $(,$rest_a, $rest_b)*)
    }};
    ($chain_rel: ident) => {
        Continue(())
    };
}

macro_rules! lexical_partial_cmp {
    ($a:expr, $b:expr, $($rest_a:expr, $rest_b:expr),+) => {
        match ($a).partial_cmp(&$b) {
            Some(Equal) => lexical_partial_cmp!($($rest_a, $rest_b),+),
            ordering => ordering
        }
    };
    ($a:expr, $b:expr) => { ($a).partial_cmp(&$b) };
}

macro_rules! lexical_cmp {
    ($a:expr, $b:expr, $($rest_a:expr, $rest_b:expr),+) => {
        match ($a).cmp(&$b) {
            Equal => lexical_cmp!($($rest_a, $rest_b),+),
            ordering => ordering
        }
    };
    ($a:expr, $b:expr) => { ($a).cmp(&$b) };
}

tuple_impls!(E D C B A Z Y X W V U T);
