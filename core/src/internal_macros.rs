// 基于 "op T" 实现一元运算符 "op &T"，
// 其中 T 预期可 `Copy`。
macro_rules! forward_ref_unop {
    (impl $imp:ident, $method:ident for $t:ty, $(#[$attr:meta])+) => {
        $(#[$attr])+
        impl const $imp for &$t {
            type Output = <$t as $imp>::Output;

            #[inline]
            fn $method(self) -> <$t as $imp>::Output {
                $imp::$method(*self)
            }
        }
    }
}

// 基于 "T op U" 实现二元运算符 "&T op U"、"T op &U"、"&T op &U"，
// 其中 T 和 U 预期可 `Copy`。
macro_rules! forward_ref_binop {
    (impl $imp:ident, $method:ident for $t:ty, $u:ty, $(#[$attr:meta])+) => {
        $(#[$attr])+
        impl const $imp<$u> for &$t {
            type Output = <$t as $imp<$u>>::Output;

            #[inline]
            #[track_caller]
            fn $method(self, other: $u) -> <$t as $imp<$u>>::Output {
                $imp::$method(*self, other)
            }
        }

        $(#[$attr])+
        impl const $imp<&$u> for $t {
            type Output = <$t as $imp<$u>>::Output;

            #[inline]
            #[track_caller]
            fn $method(self, other: &$u) -> <$t as $imp<$u>>::Output {
                $imp::$method(self, *other)
            }
        }

        $(#[$attr])+
        impl const $imp<&$u> for &$t {
            type Output = <$t as $imp<$u>>::Output;

            #[inline]
            #[track_caller]
            fn $method(self, other: &$u) -> <$t as $imp<$u>>::Output {
                $imp::$method(*self, *other)
            }
        }
    }
}

// 基于 "T op= U" 实现 "T op= &U"，
// 其中 U 预期可 `Copy`。
macro_rules! forward_ref_op_assign {
    (impl $imp:ident, $method:ident for $t:ty, $u:ty, $(#[$attr:meta])+) => {
        $(#[$attr])+
        impl const $imp<&$u> for $t {
            #[inline]
            #[track_caller]
            fn $method(&mut self, other: &$u) {
                $imp::$method(self, *other);
            }
        }
    }
}

/// 创建一个类似闭包类型但带有名称的零大小类型。
macro_rules! impl_fn_for_zst {
    ($(
        $( #[$attr: meta] )*
        struct $Name: ident impl$( <$( $lifetime : lifetime ),+> )? Fn =
            |$( $arg: ident: $ArgTy: ty ),*| -> $ReturnTy: ty
            $body: block;
    )+) => {
        $(
            $( #[$attr] )*
            struct $Name;

            impl $( <$( $lifetime ),+> )? Fn<($( $ArgTy, )*)> for $Name {
                #[inline]
                extern "rust-call" fn call(&self, ($( $arg, )*): ($( $ArgTy, )*)) -> $ReturnTy {
                    $body
                }
            }

            impl $( <$( $lifetime ),+> )? FnMut<($( $ArgTy, )*)> for $Name {
                #[inline]
                extern "rust-call" fn call_mut(
                    &mut self,
                    ($( $arg, )*): ($( $ArgTy, )*)
                ) -> $ReturnTy {
                    Fn::call(&*self, ($( $arg, )*))
                }
            }

            impl $( <$( $lifetime ),+> )? FnOnce<($( $ArgTy, )*)> for $Name {
                type Output = $ReturnTy;

                #[inline]
                extern "rust-call" fn call_once(self, ($( $arg, )*): ($( $ArgTy, )*)) -> $ReturnTy {
                    Fn::call(&self, ($( $arg, )*))
                }
            }
        )+
    }
}
