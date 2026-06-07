//! 用于动态类型识别(dynamic typing)或类型反射(type reflection)的工具。
//!
//! # `Any` 与 `TypeId`
//!
//! `Any` 本身可以用来获取一个 `TypeId`,而当它作为 trait 对象使用时还有
//! 更多功能。作为 `&dyn Any`(一个借用的 trait 对象),它具有 `is` 和
//! `downcast_ref` 方法,用来检验所包含的值是否为给定类型,以及把内部值当作
//! 某个类型来获取它的引用。作为 `&mut dyn Any`,还有 `downcast_mut` 方法,
//! 用来获取内部值的可变引用。`Box<dyn Any>` 增加了 `downcast` 方法,它尝试
//! 转换成 `Box<T>`。完整细节参见 [`Box`] 文档。
//!
//! 注意,`&dyn Any` 仅限于检验一个值是否为某个指定的 *具体* 类型,而不能
//! 用来检验一个类型是否实现了某个 trait。这正是基于 `TypeId` 的运行期类型
//! 识别的局限之一:它只能反映精确的具体类型,无法反映 trait 实现关系或泛型
//! 之间的关系。
//!
//! [`Box`]: ../../std/boxed/struct.Box.html
//!
//! # 智能指针与 `dyn Any`
//!
//! 当把 `Any` 当作 trait 对象使用时(尤其是与 `Box<dyn Any>` 或
//! `Arc<dyn Any>` 这类类型一起),有一点行为需要牢记:对值简单地调用
//! `.type_id()`,得到的是 *容器* 的 `TypeId`,而不是底层 trait 对象的。
//! 要避免这一点,可以改为把智能指针转换成 `&dyn Any`,这样就会返回该对象
//! 的 `TypeId`。例如:
//!
//! ```
//! use std::any::{Any, TypeId};
//!
//! let boxed: Box<dyn Any> = Box::new(3_i32);
//!
//! // 你更可能想要的是这个:
//! let actual_id = (&*boxed).type_id();
//! // ……而不是这个:
//! let boxed_id = boxed.type_id();
//!
//! assert_eq!(actual_id, TypeId::of::<i32>());
//! assert_eq!(boxed_id, TypeId::of::<Box<dyn Any>>());
//! ```
//!
//! ## 示例
//!
//! 设想这样一种情形:我们想把传给某个函数的值记录到日志中。我们知道正在
//! 处理的值实现了 `Debug`,但不知道它的具体类型。我们想对某些类型给予特殊
//! 处理:在本例中,对 `String` 值,在打印其值之前先打印它的长度。我们在
//! 编译期并不知道值的具体类型,因此需要改用运行期反射。
//!
//! ```rust
//! use std::fmt::Debug;
//! use std::any::Any;
//!
//! // 面向任何实现了 `Debug` 的类型的日志函数。
//! fn log<T: Any + Debug>(value: &T) {
//!     let value_any = value as &dyn Any;
//!
//!     // 尝试把我们的值转换成 `String`。如果成功,我们想把 `String` 的长度
//!     // 连同它的值一起输出。如果失败,说明它是另一种类型:直接原样打印出来。
//!     match value_any.downcast_ref::<String>() {
//!         Some(as_string) => {
//!             println!("String ({}): {}", as_string.len(), as_string);
//!         }
//!         None => {
//!             println!("{value:?}");
//!         }
//!     }
//! }
//!
//! // 这个函数想在对其参数做处理之前,先把参数记录到日志。
//! fn do_work<T: Any + Debug>(value: &T) {
//!     log(value);
//!     // ……做些别的工作
//! }
//!
//! fn main() {
//!     let my_string = "Hello World".to_string();
//!     do_work(&my_string);
//!
//!     let my_i8: i8 = 100;
//!     do_work(&my_i8);
//! }
//! ```
//!

#![stable(feature = "rust1", since = "1.0.0")]

use crate::{fmt, hash, intrinsics, ptr};

///////////////////////////////////////////////////////////////////////////////
// Any trait
///////////////////////////////////////////////////////////////////////////////

/// 一个用来模拟动态类型识别的 trait。
///
/// 大多数类型都实现了 `Any`。然而,任何含有非 `'static` 引用的类型都不实现它。
/// 更多细节参见[模块级文档][mod]。这也是 `Any` 的一大局限:只有 `'static`
/// 类型(即不借用任何短于 `'static` 生命周期的数据的类型)才能实现 `Any`。
///
/// [mod]: crate::any
// 本 trait 不是 unsafe 的,尽管我们在 unsafe 代码中(例如 `downcast`)依赖了
// 它唯一那个 impl 的 `type_id` 函数的具体行为。通常这会是个问题,但由于 `Any`
// 的唯一 impl 是一个覆盖性(blanket)实现,所以没有别的代码能实现 `Any`。
//
// 我们其实大可以把本 trait 做成 unsafe 的——这不会造成破坏,因为我们掌控着
// 所有实现——但我们选择不这么做,因为这既无甚必要,又可能让用户对 unsafe
// trait 与 unsafe 方法之间的区别产生混淆(也就是说,`type_id` 调用起来仍然
// 是安全的,但我们大概会想在文档中标明这一点)。
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "Any"]
pub trait Any: 'static {
    /// 获取 `self` 的 `TypeId`。
    ///
    /// 如果在一个 `dyn Any` trait 对象(或 `Any` 的某个子 trait 的 trait
    /// 对象)上调用,它返回的是底层 *具体* 类型的 `TypeId`,而不是
    /// `dyn Any` 本身的。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::any::{Any, TypeId};
    ///
    /// fn is_string(s: &dyn Any) -> bool {
    ///     TypeId::of::<String>() == s.type_id()
    /// }
    ///
    /// assert_eq!(is_string(&0), false);
    /// assert_eq!(is_string(&"cookie monster".to_string()), true);
    /// ```
    #[stable(feature = "get_type_id", since = "1.34.0")]
    fn type_id(&self) -> TypeId;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: 'static + ?Sized> Any for T {
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
}

///////////////////////////////////////////////////////////////////////////////
// Any trait 对象的扩展方法。
///////////////////////////////////////////////////////////////////////////////

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for dyn Any {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Any").finish_non_exhaustive()
    }
}

// 确保比如 join 一个线程所得的结果可以被打印,从而能配合 `unwrap` 使用。
// 如果将来分发(dispatch)能配合 upcasting 工作,这最终也许就不再需要了。
#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for dyn Any + Send {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Any").finish_non_exhaustive()
    }
}

#[stable(feature = "any_send_sync_methods", since = "1.28.0")]
impl fmt::Debug for dyn Any + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Any").finish_non_exhaustive()
    }
}

impl dyn Any {
    /// Returns `true` if the inner type is the same as `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn is_string(s: &dyn Any) {
    ///     if s.is::<String>() {
    ///         println!("It's a string!");
    ///     } else {
    ///         println!("Not a string...");
    ///     }
    /// }
    ///
    /// is_string(&0);
    /// is_string(&"cookie monster".to_string());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn is<T: Any>(&self) -> bool {
        // Get `TypeId` of the type this function is instantiated with.
        let t = TypeId::of::<T>();

        // Get `TypeId` of the type in the trait object (`self`).
        let concrete = self.type_id();

        // Compare both `TypeId`s on equality.
        t == concrete
    }

    /// Returns some reference to the inner value if it is of type `T`, or
    /// `None` if it isn't.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn print_if_string(s: &dyn Any) {
    ///     if let Some(string) = s.downcast_ref::<String>() {
    ///         println!("It's a string({}): '{}'", string.len(), string);
    ///     } else {
    ///         println!("Not a string...");
    ///     }
    /// }
    ///
    /// print_if_string(&0);
    /// print_if_string(&"cookie monster".to_string());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        if self.is::<T>() {
            // SAFETY: just checked whether we are pointing to the correct type, and we can rely on
            // that check for memory safety because we have implemented Any for all types; no other
            // impls can exist as they would conflict with our impl.
            unsafe { Some(self.downcast_unchecked_ref()) }
        } else {
            None
        }
    }

    /// Returns some mutable reference to the inner value if it is of type `T`, or
    /// `None` if it isn't.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn modify_if_u32(s: &mut dyn Any) {
    ///     if let Some(num) = s.downcast_mut::<u32>() {
    ///         *num = 42;
    ///     }
    /// }
    ///
    /// let mut x = 10u32;
    /// let mut s = "starlord".to_string();
    ///
    /// modify_if_u32(&mut x);
    /// modify_if_u32(&mut s);
    ///
    /// assert_eq!(x, 42);
    /// assert_eq!(&s, "starlord");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            // SAFETY: just checked whether we are pointing to the correct type, and we can rely on
            // that check for memory safety because we have implemented Any for all types; no other
            // impls can exist as they would conflict with our impl.
            unsafe { Some(self.downcast_unchecked_mut()) }
        } else {
            None
        }
    }

    /// Returns a reference to the inner value as type `dyn T`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let x: Box<dyn Any> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     assert_eq!(*x.downcast_unchecked_ref::<usize>(), 1);
    /// }
    /// ```
    ///
    /// # Safety
    ///
    /// The contained value must be of type `T`. Calling this method
    /// with the incorrect type is *undefined behavior*.
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_unchecked_ref<T: Any>(&self) -> &T {
        debug_assert!(self.is::<T>());
        // SAFETY: caller guarantees that T is the correct type
        unsafe { &*(self as *const dyn Any as *const T) }
    }

    /// Returns a mutable reference to the inner value as type `dyn T`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let mut x: Box<dyn Any> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     *x.downcast_unchecked_mut::<usize>() += 1;
    /// }
    ///
    /// assert_eq!(*x.downcast_ref::<usize>().unwrap(), 2);
    /// ```
    ///
    /// # Safety
    ///
    /// The contained value must be of type `T`. Calling this method
    /// with the incorrect type is *undefined behavior*.
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_unchecked_mut<T: Any>(&mut self) -> &mut T {
        debug_assert!(self.is::<T>());
        // SAFETY: caller guarantees that T is the correct type
        unsafe { &mut *(self as *mut dyn Any as *mut T) }
    }
}

impl dyn Any + Send {
    /// Forwards to the method defined on the type `dyn Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn is_string(s: &(dyn Any + Send)) {
    ///     if s.is::<String>() {
    ///         println!("It's a string!");
    ///     } else {
    ///         println!("Not a string...");
    ///     }
    /// }
    ///
    /// is_string(&0);
    /// is_string(&"cookie monster".to_string());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn is<T: Any>(&self) -> bool {
        <dyn Any>::is::<T>(self)
    }

    /// Forwards to the method defined on the type `dyn Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn print_if_string(s: &(dyn Any + Send)) {
    ///     if let Some(string) = s.downcast_ref::<String>() {
    ///         println!("It's a string({}): '{}'", string.len(), string);
    ///     } else {
    ///         println!("Not a string...");
    ///     }
    /// }
    ///
    /// print_if_string(&0);
    /// print_if_string(&"cookie monster".to_string());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        <dyn Any>::downcast_ref::<T>(self)
    }

    /// Forwards to the method defined on the type `dyn Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn modify_if_u32(s: &mut (dyn Any + Send)) {
    ///     if let Some(num) = s.downcast_mut::<u32>() {
    ///         *num = 42;
    ///     }
    /// }
    ///
    /// let mut x = 10u32;
    /// let mut s = "starlord".to_string();
    ///
    /// modify_if_u32(&mut x);
    /// modify_if_u32(&mut s);
    ///
    /// assert_eq!(x, 42);
    /// assert_eq!(&s, "starlord");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        <dyn Any>::downcast_mut::<T>(self)
    }

    /// Forwards to the method defined on the type `dyn Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let x: Box<dyn Any> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     assert_eq!(*x.downcast_unchecked_ref::<usize>(), 1);
    /// }
    /// ```
    ///
    /// # Safety
    ///
    /// The contained value must be of type `T`. Calling this method
    /// with the incorrect type is *undefined behavior*.
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_unchecked_ref<T: Any>(&self) -> &T {
        // SAFETY: guaranteed by caller
        unsafe { <dyn Any>::downcast_unchecked_ref::<T>(self) }
    }

    /// Forwards to the method defined on the type `dyn Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let mut x: Box<dyn Any> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     *x.downcast_unchecked_mut::<usize>() += 1;
    /// }
    ///
    /// assert_eq!(*x.downcast_ref::<usize>().unwrap(), 2);
    /// ```
    ///
    /// # Safety
    ///
    /// The contained value must be of type `T`. Calling this method
    /// with the incorrect type is *undefined behavior*.
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_unchecked_mut<T: Any>(&mut self) -> &mut T {
        // SAFETY: guaranteed by caller
        unsafe { <dyn Any>::downcast_unchecked_mut::<T>(self) }
    }
}

impl dyn Any + Send + Sync {
    /// Forwards to the method defined on the type `Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn is_string(s: &(dyn Any + Send + Sync)) {
    ///     if s.is::<String>() {
    ///         println!("It's a string!");
    ///     } else {
    ///         println!("Not a string...");
    ///     }
    /// }
    ///
    /// is_string(&0);
    /// is_string(&"cookie monster".to_string());
    /// ```
    #[stable(feature = "any_send_sync_methods", since = "1.28.0")]
    #[inline]
    pub fn is<T: Any>(&self) -> bool {
        <dyn Any>::is::<T>(self)
    }

    /// Forwards to the method defined on the type `Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn print_if_string(s: &(dyn Any + Send + Sync)) {
    ///     if let Some(string) = s.downcast_ref::<String>() {
    ///         println!("It's a string({}): '{}'", string.len(), string);
    ///     } else {
    ///         println!("Not a string...");
    ///     }
    /// }
    ///
    /// print_if_string(&0);
    /// print_if_string(&"cookie monster".to_string());
    /// ```
    #[stable(feature = "any_send_sync_methods", since = "1.28.0")]
    #[inline]
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        <dyn Any>::downcast_ref::<T>(self)
    }

    /// Forwards to the method defined on the type `Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn modify_if_u32(s: &mut (dyn Any + Send + Sync)) {
    ///     if let Some(num) = s.downcast_mut::<u32>() {
    ///         *num = 42;
    ///     }
    /// }
    ///
    /// let mut x = 10u32;
    /// let mut s = "starlord".to_string();
    ///
    /// modify_if_u32(&mut x);
    /// modify_if_u32(&mut s);
    ///
    /// assert_eq!(x, 42);
    /// assert_eq!(&s, "starlord");
    /// ```
    #[stable(feature = "any_send_sync_methods", since = "1.28.0")]
    #[inline]
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        <dyn Any>::downcast_mut::<T>(self)
    }

    /// Forwards to the method defined on the type `Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let x: Box<dyn Any> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     assert_eq!(*x.downcast_unchecked_ref::<usize>(), 1);
    /// }
    /// ```
    /// # Safety
    ///
    /// The contained value must be of type `T`. Calling this method
    /// with the incorrect type is *undefined behavior*.
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_unchecked_ref<T: Any>(&self) -> &T {
        // SAFETY: guaranteed by caller
        unsafe { <dyn Any>::downcast_unchecked_ref::<T>(self) }
    }

    /// Forwards to the method defined on the type `Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let mut x: Box<dyn Any> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     *x.downcast_unchecked_mut::<usize>() += 1;
    /// }
    ///
    /// assert_eq!(*x.downcast_ref::<usize>().unwrap(), 2);
    /// ```
    /// # Safety
    ///
    /// The contained value must be of type `T`. Calling this method
    /// with the incorrect type is *undefined behavior*.
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_unchecked_mut<T: Any>(&mut self) -> &mut T {
        // SAFETY: guaranteed by caller
        unsafe { <dyn Any>::downcast_unchecked_mut::<T>(self) }
    }
}

///////////////////////////////////////////////////////////////////////////////
// TypeID and its methods
///////////////////////////////////////////////////////////////////////////////

/// A `TypeId` represents a globally unique identifier for a type.
///
/// Each `TypeId` is an opaque object which does not allow inspection of what's
/// inside but does allow basic operations such as cloning, comparison,
/// printing, and showing.
///
/// A `TypeId` is currently only available for types which ascribe to `'static`,
/// but this limitation may be removed in the future.
///
/// While `TypeId` implements `Hash`, `PartialOrd`, and `Ord`, it is worth
/// noting that the hashes and ordering will vary between Rust releases. Beware
/// of relying on them inside of your code!
///
/// # Layout
///
/// Like other [`Rust`-representation][repr-rust] types, `TypeId`'s size and layout are unstable.
/// In particular, this means that you cannot rely on the size and layout of `TypeId` remaining the
/// same between Rust releases; they are subject to change without prior notice between Rust
/// releases.
///
/// [repr-rust]: https://doc.rust-lang.org/reference/type-layout.html#r-layout.repr.rust.unspecified
///
/// # Danger of Improper Variance
///
/// You might think that subtyping is impossible between two static types,
/// but this is false; there exists a static type with a static subtype.
/// To wit, `fn(&str)`, which is short for `for<'any> fn(&'any str)`, and
/// `fn(&'static str)`, are two distinct, static types, and yet,
/// `fn(&str)` is a subtype of `fn(&'static str)`, since any value of type
/// `fn(&str)` can be used where a value of type `fn(&'static str)` is needed.
///
/// This means that abstractions around `TypeId`, despite its
/// `'static` bound on arguments, still need to worry about unnecessary
/// and improper variance: it is advisable to strive for invariance
/// first. The usability impact will be negligible, while the reduction
/// in the risk of unsoundness will be most welcome.
///
/// ## Examples
///
/// Suppose `SubType` is a subtype of `SuperType`, that is,
/// a value of type `SubType` can be used wherever
/// a value of type `SuperType` is expected.
/// Suppose also that `CoVar<T>` is a generic type, which is covariant over `T`
/// (like many other types, including `PhantomData<T>` and `Vec<T>`).
///
/// Then, by covariance, `CoVar<SubType>` is a subtype of `CoVar<SuperType>`,
/// that is, a value of type `CoVar<SubType>` can be used wherever
/// a value of type `CoVar<SuperType>` is expected.
///
/// Then if `CoVar<SuperType>` relies on `TypeId::of::<SuperType>()` to uphold any invariants,
/// those invariants may be broken because a value of type `CoVar<SuperType>` can be created
/// without going through any of its methods, like so:
/// ```
/// type SubType = fn(&());
/// type SuperType = fn(&'static ());
/// type CoVar<T> = Vec<T>; // imagine something more complicated
///
/// let sub: CoVar<SubType> = CoVar::new();
/// // we have a `CoVar<SuperType>` instance without
/// // *ever* having called `CoVar::<SuperType>::new()`!
/// let fake_super: CoVar<SuperType> = sub;
/// ```
///
/// The following is an example program that tries to use `TypeId::of` to
/// implement a generic type `Unique<T>` that guarantees unique instances for each `Unique<T>`,
/// that is, and for each type `T` there can be at most one value of type `Unique<T>` at any time.
///
/// ```
/// mod unique {
///     use std::any::TypeId;
///     use std::collections::BTreeSet;
///     use std::marker::PhantomData;
///     use std::sync::Mutex;
///
///     static ID_SET: Mutex<BTreeSet<TypeId>> = Mutex::new(BTreeSet::new());
///
///     // TypeId has only covariant uses, which makes Unique covariant over TypeAsId 🚨
///     #[derive(Debug, PartialEq)]
///     pub struct Unique<TypeAsId: 'static>(
///         // private field prevents creation without `new` outside this module
///         PhantomData<TypeAsId>,
///     );
///
///     impl<TypeAsId: 'static> Unique<TypeAsId> {
///         pub fn new() -> Option<Self> {
///             let mut set = ID_SET.lock().unwrap();
///             (set.insert(TypeId::of::<TypeAsId>())).then(|| Self(PhantomData))
///         }
///     }
///
///     impl<TypeAsId: 'static> Drop for Unique<TypeAsId> {
///         fn drop(&mut self) {
///             let mut set = ID_SET.lock().unwrap();
///             (!set.remove(&TypeId::of::<TypeAsId>())).then(|| panic!("duplicity detected"));
///         }
///     }
/// }
///
/// use unique::Unique;
///
/// // `OtherRing` is a subtype of `TheOneRing`. Both are 'static, and thus have a TypeId.
/// type TheOneRing = fn(&'static ());
/// type OtherRing = fn(&());
///
/// fn main() {
///     let the_one_ring: Unique<TheOneRing> = Unique::new().unwrap();
///     assert_eq!(Unique::<TheOneRing>::new(), None);
///
///     let other_ring: Unique<OtherRing> = Unique::new().unwrap();
///     // Use that `Unique<OtherRing>` is a subtype of `Unique<TheOneRing>` 🚨
///     let fake_one_ring: Unique<TheOneRing> = other_ring;
///     assert_eq!(fake_one_ring, the_one_ring);
///
///     std::mem::forget(fake_one_ring);
/// }
/// ```
#[derive(Copy, PartialOrd, Ord)]
#[derive_const(Clone, Eq)]
#[stable(feature = "rust1", since = "1.0.0")]
#[lang = "type_id"]
pub struct TypeId {
    /// This needs to be an array of pointers, since there is provenance
    /// in the first array field. This provenance knows exactly which type
    /// the TypeId actually is, allowing CTFE and miri to operate based off it.
    /// At runtime all the pointers in the array contain bits of the hash, making
    /// the entire `TypeId` actually just be a `u128` hash of the type.
    pub(crate) data: [*const (); 16 / size_of::<*const ()>()],
}

// SAFETY: the raw pointer is always an integer
#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl Send for TypeId {}
// SAFETY: the raw pointer is always an integer
#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl Sync for TypeId {}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl const PartialEq for TypeId {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        #[cfg(miri)]
        return crate::intrinsics::type_id_eq(*self, *other);
        #[cfg(not(miri))]
        {
            let this = self;
            crate::intrinsics::const_eval_select!(
                @capture { this: &TypeId, other: &TypeId } -> bool:
                if const {
                    crate::intrinsics::type_id_eq(*this, *other)
                } else {
                    // Ideally we would just invoke `type_id_eq` unconditionally here,
                    // but since we do not MIR inline intrinsics, because backends
                    // may want to override them (and miri does!), MIR opts do not
                    // clean up this call sufficiently for LLVM to turn repeated calls
                    // of `TypeId` comparisons against one specific `TypeId` into
                    // a lookup table.
                    // SAFETY: We know that at runtime none of the bits have provenance and all bits
                    // are initialized. So we can just convert the whole thing to a `u128` and compare that.
                    unsafe {
                        crate::mem::transmute::<_, u128>(*this) == crate::mem::transmute::<_, u128>(*other)
                    }
                }
            )
        }
    }
}

impl TypeId {
    /// Returns the `TypeId` of the generic type parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::{Any, TypeId};
    ///
    /// fn is_string<T: ?Sized + Any>(_s: &T) -> bool {
    ///     TypeId::of::<String>() == TypeId::of::<T>()
    /// }
    ///
    /// assert_eq!(is_string(&0), false);
    /// assert_eq!(is_string(&"cookie monster".to_string()), true);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_type_id", since = "1.91.0")]
    pub const fn of<T: ?Sized + 'static>() -> TypeId {
        const { intrinsics::type_id::<T>() }
    }

    fn as_u128(self) -> u128 {
        let mut bytes = [0; 16];

        // This is a provenance-stripping memcpy.
        for (i, chunk) in self.data.iter().copied().enumerate() {
            let chunk = chunk.addr().to_ne_bytes();
            let start = i * chunk.len();
            bytes[start..(start + chunk.len())].copy_from_slice(&chunk);
        }
        u128::from_ne_bytes(bytes)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl hash::Hash for TypeId {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        // We only hash the lower 64 bits of our (128 bit) internal numeric ID,
        // because:
        // - The hashing algorithm which backs `TypeId` is expected to be
        //   unbiased and high quality, meaning further mixing would be somewhat
        //   redundant compared to choosing (the lower) 64 bits arbitrarily.
        // - `Hasher::finish` returns a u64 anyway, so the extra entropy we'd
        //   get from hashing the full value would probably not be useful
        //   (especially given the previous point about the lower 64 bits being
        //   high quality on their own).
        // - It is correct to do so -- only hashing a subset of `self` is still
        //   compatible with an `Eq` implementation that considers the entire
        //   value, as ours does.
        let data =
        // SAFETY: The `offset` stays in-bounds, it just moves the pointer to the 2nd half of the `TypeId`.
        // Only the first ptr-sized chunk ever has provenance, so that second half is always
        // fine to read at integer type.
            unsafe { crate::ptr::read_unaligned(self.data.as_ptr().cast::<u64>().offset(1)) };
        data.hash(state);
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for TypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "TypeId({:#034x})", self.as_u128())
    }
}

/// Returns the name of a type as a string slice.
///
/// # Note
///
/// This is intended for diagnostic use. The exact contents and format of the
/// string returned are not specified, other than being a best-effort
/// description of the type. For example, amongst the strings
/// that `type_name::<Option<String>>()` might return are `"Option<String>"` and
/// `"std::option::Option<std::string::String>"`.
///
/// The returned string must not be considered to be a unique identifier of a
/// type as multiple types may map to the same type name. Similarly, there is no
/// guarantee that all parts of a type will appear in the returned string. In
/// addition, the output may change between versions of the compiler. For
/// example, lifetime specifiers were omitted in some earlier versions.
///
/// The current implementation uses the same infrastructure as compiler
/// diagnostics and debuginfo, but this is not guaranteed.
///
/// # Examples
///
/// ```rust
/// assert_eq!(
///     std::any::type_name::<Option<String>>(),
///     "core::option::Option<alloc::string::String>",
/// );
/// ```
#[must_use]
#[stable(feature = "type_name", since = "1.38.0")]
#[rustc_const_unstable(feature = "const_type_name", issue = "63084")]
pub const fn type_name<T: ?Sized>() -> &'static str {
    const { intrinsics::type_name::<T>() }
}

/// Returns the type name of the pointed-to value as a string slice.
///
/// This is the same as `type_name::<T>()`, but can be used where the type of a
/// variable is not easily available.
///
/// # Note
///
/// Like [`type_name`], this is intended for diagnostic use and the exact output is not
/// guaranteed. It provides a best-effort description, but the output may change between
/// versions of the compiler.
///
/// In short: use this for debugging, avoid using the output to affect program behavior. More
/// information is available at [`type_name`].
///
/// Additionally, this function does not resolve trait objects. This means that
/// `type_name_of_val(&7u32 as &dyn Debug)` may return `"dyn Debug"`, but will not return `"u32"`
/// at this time.
///
/// # Examples
///
/// Prints the default integer and float types.
///
/// ```rust
/// use std::any::type_name_of_val;
///
/// let s = "foo";
/// let x: i32 = 1;
/// let y: f32 = 1.0;
///
/// assert!(type_name_of_val(&s).contains("str"));
/// assert!(type_name_of_val(&x).contains("i32"));
/// assert!(type_name_of_val(&y).contains("f32"));
/// ```
#[must_use]
#[stable(feature = "type_name_of_val", since = "1.76.0")]
#[rustc_const_unstable(feature = "const_type_name", issue = "63084")]
pub const fn type_name_of_val<T: ?Sized>(_val: &T) -> &'static str {
    type_name::<T>()
}

/// Returns `Some(&U)` if `T` can be coerced to the trait object type `U`. Otherwise, it returns `None`.
///
/// # Compile-time failures
/// Determining whether `T` can be coerced to the trait object type `U` requires compiler trait resolution.
/// In some cases, that resolution can exceed the recursion limit,
/// and compilation will fail instead of this function returning `None`.
/// # Examples
///
/// ```rust
/// #![feature(try_as_dyn)]
///
/// use core::any::try_as_dyn;
///
/// trait Animal {
///     fn speak(&self) -> &'static str;
/// }
///
/// struct Dog;
/// impl Animal for Dog {
///     fn speak(&self) -> &'static str { "woof" }
/// }
///
/// struct Rock; // does not implement Animal
///
/// let dog = Dog;
/// let rock = Rock;
///
/// let as_animal: Option<&dyn Animal> = try_as_dyn::<Dog, dyn Animal>(&dog);
/// assert_eq!(as_animal.unwrap().speak(), "woof");
///
/// let not_an_animal: Option<&dyn Animal> = try_as_dyn::<Rock, dyn Animal>(&rock);
/// assert!(not_an_animal.is_none());
/// ```
#[must_use]
#[unstable(feature = "try_as_dyn", issue = "144361")]
pub const fn try_as_dyn<
    T: Any + 'static,
    U: ptr::Pointee<Metadata = ptr::DynMetadata<U>> + ?Sized + 'static,
>(
    t: &T,
) -> Option<&U> {
    let vtable: Option<ptr::DynMetadata<U>> = const { intrinsics::vtable_for::<T, U>() };
    match vtable {
        Some(dyn_metadata) => {
            let pointer = ptr::from_raw_parts(t, dyn_metadata);
            // SAFETY: `t` is a reference to a type, so we know it is valid.
            // `dyn_metadata` is a vtable for T, implementing the trait of `U`.
            Some(unsafe { &*pointer })
        }
        None => None,
    }
}

/// Returns `Some(&mut U)` if `T` can be coerced to the trait object type `U`. Otherwise, it returns `None`.
///
/// # Compile-time failures
/// Determining whether `T` can be coerced to the trait object type `U` requires compiler trait resolution.
/// In some cases, that resolution can exceed the recursion limit,
/// and compilation will fail instead of this function returning `None`.
/// # Examples
///
/// ```rust
/// #![feature(try_as_dyn)]
///
/// use core::any::try_as_dyn_mut;
///
/// trait Animal {
///     fn speak(&self) -> &'static str;
/// }
///
/// struct Dog;
/// impl Animal for Dog {
///     fn speak(&self) -> &'static str { "woof" }
/// }
///
/// struct Rock; // does not implement Animal
///
/// let mut dog = Dog;
/// let mut rock = Rock;
///
/// let as_animal: Option<&mut dyn Animal> = try_as_dyn_mut::<Dog, dyn Animal>(&mut dog);
/// assert_eq!(as_animal.unwrap().speak(), "woof");
///
/// let not_an_animal: Option<&mut dyn Animal> = try_as_dyn_mut::<Rock, dyn Animal>(&mut rock);
/// assert!(not_an_animal.is_none());
/// ```
#[must_use]
#[unstable(feature = "try_as_dyn", issue = "144361")]
pub const fn try_as_dyn_mut<
    T: Any + 'static,
    U: ptr::Pointee<Metadata = ptr::DynMetadata<U>> + ?Sized + 'static,
>(
    t: &mut T,
) -> Option<&mut U> {
    let vtable: Option<ptr::DynMetadata<U>> = const { intrinsics::vtable_for::<T, U>() };
    match vtable {
        Some(dyn_metadata) => {
            let pointer = ptr::from_raw_parts_mut(t, dyn_metadata);
            // SAFETY: `t` is a reference to a type, so we know it is valid.
            // `dyn_metadata` is a vtable for T, implementing the trait of `U`.
            Some(unsafe { &mut *pointer })
        }
        None => None,
    }
}
