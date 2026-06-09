#![unstable(feature = "phantom_variance_markers", issue = "135806")]

use super::PhantomData;
use crate::any::type_name;
use crate::clone::TrivialClone;
use crate::cmp::Ordering;
use crate::fmt;
use crate::hash::{Hash, Hasher};

macro_rules! first_token {
    ($first:tt $($rest:tt)*) => {
        $first
    };
}

macro_rules! phantom_type {
    ($(
        $(#[$attr:meta])*
        pub struct $name:ident <$t:ident> ($($inner:tt)*);
    )*) => {$(
        $(#[$attr])*
        pub struct $name<$t>($($inner)*) where $t: ?Sized;

        impl<T> $name<T>
            where T: ?Sized
        {
            /// 构造该变型(variance)标记的一个新实例。
            pub const fn new() -> Self {
                Self(PhantomData)
            }
        }

        impl<T> self::sealed::Sealed for $name<T> where T: ?Sized {
            const VALUE: Self = Self::new();
        }
        impl<T> Variance for $name<T> where T: ?Sized {}

        impl<T> Default for $name<T>
            where T: ?Sized
        {
            fn default() -> Self {
                Self(PhantomData)
            }
        }

        impl<T> fmt::Debug for $name<T>
            where T: ?Sized
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}<{}>", stringify!($name), type_name::<T>())
            }
        }

        impl<T> Clone for $name<T>
            where T: ?Sized
        {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<T> Copy for $name<T> where T: ?Sized {}

        #[doc(hidden)]
        unsafe impl<T> TrivialClone for $name<T> where T: ?Sized {}

        impl<T> PartialEq for $name<T>
            where T: ?Sized
        {
            fn eq(&self, _: &Self) -> bool {
                true
            }
        }

        impl<T> Eq for $name<T> where T: ?Sized {}

        impl<T> PartialOrd for $name<T>
            where T: ?Sized
        {
            fn partial_cmp(&self, _: &Self) -> Option<Ordering> {
                Some(Ordering::Equal)
            }
        }

        impl<T> Ord for $name<T>
            where T: ?Sized
        {
            fn cmp(&self, _: &Self) -> Ordering {
                Ordering::Equal
            }
        }

        impl<T> Hash for $name<T>
            where T: ?Sized
        {
            fn hash<H: Hasher>(&self, _: &mut H) {}
        }
    )*};
}

macro_rules! phantom_lifetime {
    ($(
        $(#[$attr:meta])*
        pub struct $name:ident <$lt:lifetime> ($($inner:tt)*);
    )*) => {$(
        $(#[$attr])*
        #[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name<$lt>($($inner)*);

        impl $name<'_> {
            /// 构造该变型(variance)标记的一个新实例。
            pub const fn new() -> Self {
                Self(first_token!($($inner)*)(PhantomData))
            }
        }

        impl self::sealed::Sealed for $name<'_> {
            const VALUE: Self = Self::new();
        }
        impl Variance for $name<'_> {}

        impl fmt::Debug for $name<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", stringify!($name))
            }
        }
    )*};
}

phantom_lifetime! {
    /// 零大小类型,用来把一个生命周期标记为协变(covariant)。
    ///
    /// 协变的生命周期必须至少活得和声明的一样长。更多信息参见[参考手册][1]。
    ///
    /// [1]: https://doc.rust-lang.org/stable/reference/subtyping.html#variance
    ///
    /// 注意:如果 `'a` 在别处是逆变或不变的,那么最终得到的类型是不变(invariant)的。
    ///
    /// ## 内存布局
    ///
    /// 对所有 `'a`,以下保证均成立:
    /// * `size_of::<PhantomCovariantLifetime<'a>>() == 0`
    /// * `align_of::<PhantomCovariantLifetime<'a>>() == 1`
    #[rustc_pub_transparent]
    #[repr(transparent)]
    pub struct PhantomCovariantLifetime<'a>(PhantomCovariant<&'a ()>);
    /// 零大小类型,用来把一个生命周期标记为逆变(contravariant)。
    ///
    /// 逆变的生命周期最多只能活得和声明的一样长。更多信息参见[参考手册][1]。
    ///
    /// [1]: https://doc.rust-lang.org/stable/reference/subtyping.html#variance
    ///
    /// 注意:如果 `'a` 在别处是协变或不变的,那么最终得到的类型是不变(invariant)的。
    ///
    /// ## 内存布局
    ///
    /// 对所有 `'a`,以下保证均成立:
    /// * `size_of::<PhantomContravariantLifetime<'a>>() == 0`
    /// * `align_of::<PhantomContravariantLifetime<'a>>() == 1`
    #[rustc_pub_transparent]
    #[repr(transparent)]
    pub struct PhantomContravariantLifetime<'a>(PhantomContravariant<&'a ()>);
    /// 零大小类型,用来把一个生命周期标记为不变(invariant)。
    ///
    /// 不变的生命周期必须恰好活得和声明的一样长,既不能更短,也不能更长。
    /// 更多信息参见[参考手册][1]。
    ///
    /// [1]: https://doc.rust-lang.org/stable/reference/subtyping.html#variance
    ///
    /// ## 内存布局
    ///
    /// 对所有 `'a`,以下保证均成立:
    /// * `size_of::<PhantomInvariantLifetime<'a>>() == 0`
    /// * `align_of::<PhantomInvariantLifetime<'a>>() == 1`
    #[rustc_pub_transparent]
    #[repr(transparent)]
    pub struct PhantomInvariantLifetime<'a>(PhantomInvariant<&'a ()>);
}

phantom_type! {
    /// 零大小类型,用来把一个类型参数标记为协变(covariant)。
    ///
    /// 用作函数返回值一部分的类型是协变的。如果该类型 _同时_ 还作为参数传入,
    /// 那么它就是[不变的][PhantomInvariant]。更多信息参见[参考手册][1]。
    ///
    /// [1]: https://doc.rust-lang.org/stable/reference/subtyping.html#variance
    ///
    /// 注意:如果 `T` 在别处是逆变或不变的,那么最终得到的类型是不变(invariant)的。
    ///
    /// ## 内存布局
    ///
    /// 对所有 `T`,以下保证均成立:
    /// * `size_of::<PhantomCovariant<T>>() == 0`
    /// * `align_of::<PhantomCovariant<T>>() == 1`
    #[rustc_pub_transparent]
    #[repr(transparent)]
    pub struct PhantomCovariant<T>(PhantomData<fn() -> T>);
    /// 零大小类型,用来把一个类型参数标记为逆变(contravariant)。
    ///
    /// 作为参数传给函数的类型是逆变的。如果该类型 _同时_ 还是函数返回值的一
    /// 部分,那么它就是[不变的][PhantomInvariant]。更多信息参见[参考手册][1]。
    ///
    /// [1]: https://doc.rust-lang.org/stable/reference/subtyping.html#variance
    ///
    /// 注意:如果 `T` 在别处是协变或不变的,那么最终得到的类型是不变(invariant)的。
    ///
    /// ## 内存布局
    ///
    /// 对所有 `T`,以下保证均成立:
    /// * `size_of::<PhantomContravariant<T>>() == 0`
    /// * `align_of::<PhantomContravariant<T>>() == 1`
    #[rustc_pub_transparent]
    #[repr(transparent)]
    pub struct PhantomContravariant<T>(PhantomData<fn(T)>);
    /// 零大小类型,用来把一个类型参数标记为不变(invariant)。
    ///
    /// 既作为参数传入 _又_ 用作函数返回值一部分的类型是不变的。更多信息参见
    /// [参考手册][1]。
    ///
    /// [1]: https://doc.rust-lang.org/stable/reference/subtyping.html#variance
    ///
    /// ## 内存布局
    ///
    /// 对所有 `T`,以下保证均成立:
    /// * `size_of::<PhantomInvariant<T>>() == 0`
    /// * `align_of::<PhantomInvariant<T>>() == 1`
    #[rustc_pub_transparent]
    #[repr(transparent)]
    pub struct PhantomInvariant<T>(PhantomData<fn(T) -> T>);
}

mod sealed {
    pub trait Sealed {
        const VALUE: Self;
    }
}

/// 用于幻影变型(phantom variance)类型的标记 trait。
pub trait Variance: sealed::Sealed + Default {}

/// 构造一个变型标记;等价于 [`Default::default`]。
///
/// 该类型可以是下列任意一种。不过,你通常不需要显式写出类型名。
///
/// - [`PhantomCovariant`]
/// - [`PhantomContravariant`]
/// - [`PhantomInvariant`]
/// - [`PhantomCovariantLifetime`]
/// - [`PhantomContravariantLifetime`]
/// - [`PhantomInvariantLifetime`]
///
/// # 示例
///
/// ```rust
/// #![feature(phantom_variance_markers)]
///
/// use core::marker::{PhantomCovariant, variance};
///
/// struct BoundFn<F, P, R>
/// where
///     F: Fn(P) -> R,
/// {
///     function: F,
///     parameter: P,
///     return_value: PhantomCovariant<R>,
/// }
///
/// let bound_fn = BoundFn {
///     function: core::convert::identity,
///     parameter: 5u8,
///     return_value: variance(),
/// };
/// ```
pub const fn variance<T>() -> T
where
    T: Variance,
{
    T::VALUE
}
