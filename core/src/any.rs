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
// Any trait(Any trait 定义)
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
    /// 如果内部类型与 `T` 相同,返回 `true`。
    ///
    /// # 示例
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
        // 取得本函数被实例化时所用类型的 `TypeId`。
        let t = TypeId::of::<T>();

        // 取得 trait 对象(`self`)中所含类型的 `TypeId`。
        let concrete = self.type_id();

        // 比较两个 `TypeId` 是否相等。
        t == concrete
    }

    /// 如果内部值的类型是 `T`,返回指向它的某个引用;否则返回 `None`。
    ///
    /// # 示例
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
            // SAFETY:我们刚刚检查过自己指向的是否是正确的类型,而且为内存
            // 安全起见,我们可以依赖这一检查,因为我们已经为所有类型实现了
            // Any;不可能存在别的 impl,否则它们会与我们的 impl 冲突。
            unsafe { Some(self.downcast_unchecked_ref()) }
        } else {
            None
        }
    }

    /// 如果内部值的类型是 `T`,返回指向它的某个可变引用;否则返回 `None`。
    ///
    /// # 示例
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
            // SAFETY:我们刚刚检查过自己指向的是否是正确的类型,而且为内存
            // 安全起见,我们可以依赖这一检查,因为我们已经为所有类型实现了
            // Any;不可能存在别的 impl,否则它们会与我们的 impl 冲突。
            unsafe { Some(self.downcast_unchecked_mut()) }
        } else {
            None
        }
    }

    /// 返回内部值作为类型 `dyn T` 的引用。
    ///
    /// # 示例
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
    /// # 安全性(Safety)
    ///
    /// 所含的值必须是 `T` 类型。以错误的类型调用本方法属于 *未定义行为*。
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_unchecked_ref<T: Any>(&self) -> &T {
        debug_assert!(self.is::<T>());
        // SAFETY:调用者保证 T 是正确的类型
        unsafe { &*(self as *const dyn Any as *const T) }
    }

    /// 返回内部值作为类型 `dyn T` 的可变引用。
    ///
    /// # 示例
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
    /// # 安全性(Safety)
    ///
    /// 所含的值必须是 `T` 类型。以错误的类型调用本方法属于 *未定义行为*。
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_unchecked_mut<T: Any>(&mut self) -> &mut T {
        debug_assert!(self.is::<T>());
        // SAFETY:调用者保证 T 是正确的类型
        unsafe { &mut *(self as *mut dyn Any as *mut T) }
    }
}

impl dyn Any + Send {
    /// 转发到类型 `dyn Any` 上定义的同名方法。
    ///
    /// # 示例
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

    /// 转发到类型 `dyn Any` 上定义的同名方法。
    ///
    /// # 示例
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

    /// 转发到类型 `dyn Any` 上定义的同名方法。
    ///
    /// # 示例
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

    /// 转发到类型 `dyn Any` 上定义的同名方法。
    ///
    /// # 示例
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
    /// # 安全性(Safety）
    ///
    /// 所含的值必须是 `T` 类型。以错误的类型调用本方法属于 *未定义行为*。
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_unchecked_ref<T: Any>(&self) -> &T {
        // SAFETY: guaranteed by caller
        unsafe { <dyn Any>::downcast_unchecked_ref::<T>(self) }
    }

    /// 转发到类型 `dyn Any` 上定义的同名方法。
    ///
    /// # 示例
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
    /// # 安全性(Safety）
    ///
    /// 所含的值必须是 `T` 类型。以错误的类型调用本方法属于 *未定义行为*。
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_unchecked_mut<T: Any>(&mut self) -> &mut T {
        // SAFETY: guaranteed by caller
        unsafe { <dyn Any>::downcast_unchecked_mut::<T>(self) }
    }
}

impl dyn Any + Send + Sync {
    /// 转发到类型 `Any` 上定义的同名方法。
    ///
    /// # 示例
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

    /// 转发到类型 `Any` 上定义的同名方法。
    ///
    /// # 示例
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

    /// 转发到类型 `Any` 上定义的同名方法。
    ///
    /// # 示例
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

    /// 转发到类型 `Any` 上定义的同名方法。
    ///
    /// # 示例
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
    /// # 安全性(Safety）
    ///
    /// 所含的值必须是 `T` 类型。以错误的类型调用本方法属于 *未定义行为*。
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_unchecked_ref<T: Any>(&self) -> &T {
        // SAFETY: guaranteed by caller
        unsafe { <dyn Any>::downcast_unchecked_ref::<T>(self) }
    }

    /// 转发到类型 `Any` 上定义的同名方法。
    ///
    /// # 示例
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
    /// # 安全性(Safety）
    ///
    /// 所含的值必须是 `T` 类型。以错误的类型调用本方法属于 *未定义行为*。
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_unchecked_mut<T: Any>(&mut self) -> &mut T {
        // SAFETY: guaranteed by caller
        unsafe { <dyn Any>::downcast_unchecked_mut::<T>(self) }
    }
}

///////////////////////////////////////////////////////////////////////////////
// TypeId 及其方法
///////////////////////////////////////////////////////////////////////////////

/// `TypeId` 代表某个类型的全局唯一标识符。
///
/// 每个 `TypeId` 都是一个不透明对象,不允许窥探其内部,但允许诸如克隆、
/// 比较、打印、展示之类的基本操作。
///
/// 目前 `TypeId` 仅对满足 `'static` 的类型可用,不过这一限制将来可能会
/// 被移除。这也是基于 `TypeId` 的运行期类型识别的根本局限:它只能用于
/// 不借用任何短于 `'static` 数据的类型。
///
/// 虽然 `TypeId` 实现了 `Hash`、`PartialOrd` 和 `Ord`,但值得注意的是:
/// 其哈希值和次序在不同 Rust 发行版之间会有所不同。切勿在你的代码中依赖它们!
///
/// # Layout
///
/// 与其他[采用 `Rust` 表示][repr-rust]的类型一样,`TypeId` 的大小和布局是
/// 不稳定的。特别地,这意味着你不能依赖 `TypeId` 的大小和布局在不同 Rust
/// 发行版之间保持不变;它们在 Rust 发行版之间可能不经预先通知就发生改变。
///
/// [repr-rust]: https://doc.rust-lang.org/reference/type-layout.html#r-layout.repr.rust.unspecified
///
/// # 不当变型(Variance）的危险
///
/// 你也许会以为两个静态类型之间不可能存在子类型关系,但这是错误的;确实存在
/// 一个拥有静态子类型的静态类型。具体来说,`fn(&str)`(它是
/// `for<'any> fn(&'any str)` 的简写)与 `fn(&'static str)` 是两个不同的静态
/// 类型,然而 `fn(&str)` 却是 `fn(&'static str)` 的子类型,因为任何
/// `fn(&str)` 类型的值都可以用在需要 `fn(&'static str)` 类型值的地方。
///
/// 这意味着,围绕 `TypeId` 构建的抽象,尽管其参数带有 `'static` 约束,仍然
/// 需要提防不必要且不当的变型:建议优先力求做到不变(invariance)。这对可用性
/// 的影响微乎其微,而它对降低不可靠(unsoundness)风险的助益则非常可贵。
///
/// ## 示例
///
/// 假设 `SubType` 是 `SuperType` 的子类型,也就是说,`SubType` 类型的值可以
/// 用在任何期望 `SuperType` 类型值的地方。再假设 `CoVar<T>` 是一个泛型类型,
/// 它对 `T` 协变(像许多其他类型一样,包括 `PhantomData<T>` 和 `Vec<T>`)。
///
/// 那么,由协变性可知,`CoVar<SubType>` 是 `CoVar<SuperType>` 的子类型,
/// 也就是说,`CoVar<SubType>` 类型的值可以用在任何期望 `CoVar<SuperType>`
/// 类型值的地方。
///
/// 这样一来,如果 `CoVar<SuperType>` 依赖 `TypeId::of::<SuperType>()` 来维持
/// 某些不变量,那么这些不变量可能会被破坏——因为一个 `CoVar<SuperType>` 类型
/// 的值可以不经由它的任何方法就被创建出来,就像这样:
/// ```
/// type SubType = fn(&());
/// type SuperType = fn(&'static ());
/// type CoVar<T> = Vec<T>; // 想象成某种更复杂的东西
///
/// let sub: CoVar<SubType> = CoVar::new();
/// // 我们 *从未* 调用过 `CoVar::<SuperType>::new()`,
/// // 却得到了一个 `CoVar<SuperType>` 实例!
/// let fake_super: CoVar<SuperType> = sub;
/// ```
///
/// 下面是一个示例程序,它试图用 `TypeId::of` 来实现一个泛型类型 `Unique<T>`,
/// 以保证每个 `Unique<T>` 的实例都唯一,也就是说,对每一个类型 `T`,在任意
/// 时刻最多只能存在一个 `Unique<T>` 类型的值。
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
///     // TypeId 只有协变的用途,这使得 Unique 对 TypeAsId 协变 🚨
///     #[derive(Debug, PartialEq)]
///     pub struct Unique<TypeAsId: 'static>(
///         // 私有字段阻止了在本模块之外不经由 `new` 就创建实例
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
/// // `OtherRing` 是 `TheOneRing` 的子类型。两者都是 'static,因此都有 TypeId。
/// type TheOneRing = fn(&'static ());
/// type OtherRing = fn(&());
///
/// fn main() {
///     let the_one_ring: Unique<TheOneRing> = Unique::new().unwrap();
///     assert_eq!(Unique::<TheOneRing>::new(), None);
///
///     let other_ring: Unique<OtherRing> = Unique::new().unwrap();
///     // 利用 `Unique<OtherRing>` 是 `Unique<TheOneRing>` 的子类型这一点 🚨
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
    /// 它必须是一个指针数组,因为第一个数组字段里带有 provenance。这份
    /// provenance 精确地知道该 TypeId 实际上对应哪个类型,从而让 CTFE 和
    /// miri 能够基于它进行运算。在运行期,数组中所有指针都装着哈希值的若干
    /// 比特,使得整个 `TypeId` 实际上就是该类型的一个 `u128` 哈希。
    pub(crate) data: [*const (); 16 / size_of::<*const ()>()],
}

// SAFETY:这个裸指针始终是一个整数
#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl Send for TypeId {}
// SAFETY:这个裸指针始终是一个整数
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
                    // 理想情况下,我们在这里会无条件地调用 `type_id_eq`,但由于
                    // 我们不对 intrinsic 做 MIR 内联(因为后端可能想要覆盖它们
                    // ——miri 就是这么做的!),MIR 优化无法把这次调用清理得足够
                    // 干净,以致 LLVM 无法把“针对某个特定 `TypeId` 反复进行
                    // `TypeId` 比较”优化成一张查找表。
                    // SAFETY:我们知道在运行期所有比特都不带 provenance,而且所有
                    // 比特都已被初始化。因此我们可以直接把整个东西转换成 `u128`
                    // 再比较。
                    unsafe {
                        crate::mem::transmute::<_, u128>(*this) == crate::mem::transmute::<_, u128>(*other)
                    }
                }
            )
        }
    }
}

impl TypeId {
    /// 返回泛型类型参数的 `TypeId`。
    ///
    /// # 示例
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

        // 这是一次会剥离 provenance 的 memcpy。
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
        // 我们只对(128 位的)内部数值 ID 的低 64 位做哈希,因为:
        // - `TypeId` 背后的哈希算法预期是无偏且高质量的,这意味着相比于
        //   任意选取(低)64 位,进一步的混合(mixing)多少有些多余。
        // - `Hasher::finish` 反正返回的是 u64,所以对整个值做哈希所多得的那点
        //   熵很可能没什么用(尤其考虑到上一点——低 64 位本身就已经是高质量的)。
        // - 这样做是正确的——只对 `self` 的一个子集做哈希,仍然与一个考虑整个
        //   值的 `Eq` 实现(我们的实现正是如此)相容。
        let data =
        // SAFETY:这个 `offset` 仍然在界内,它只是把指针挪到 `TypeId` 的后半部分。
        // 只有第一个 ptr 大小的 chunk 带有 provenance,所以后半部分总是可以安全地
        // 当作整数类型来读取。
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

/// 以字符串切片的形式返回某个类型的名字。
///
/// # 注意
///
/// 它用于诊断目的。返回字符串的确切内容和格式并未被规定,仅保证是对该类型的
/// 尽力(best-effort)描述。例如,`type_name::<Option<String>>()` 可能返回的
/// 字符串中,既有 `"Option<String>"`,也有
/// `"std::option::Option<std::string::String>"`。
///
/// 返回的字符串绝不能被当作类型的唯一标识符,因为多个类型可能映射到相同的
/// 类型名。同样地,也不保证类型的所有部分都会出现在返回的字符串中。此外,
/// 输出在不同版本的编译器之间可能会改变。例如,在某些较早的版本中,生命周期
/// 标注被省略了。
///
/// 当前的实现使用了与编译器诊断和调试信息相同的基础设施,但这并不被保证。
///
/// # 示例
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

/// 以字符串切片的形式返回被指向值的类型名。
///
/// 它与 `type_name::<T>()` 相同,但可以用在变量的类型不容易直接写出的场合。
///
/// # 注意
///
/// 与 [`type_name`] 一样,它用于诊断目的,确切的输出并不被保证。它提供的是
/// 尽力而为的描述,但输出在不同版本的编译器之间可能会改变。
///
/// 一言以蔽之:把它用于调试,避免用其输出去影响程序的行为。更多信息见
/// [`type_name`]。
///
/// 此外,本函数并不解析 trait 对象。这意味着目前
/// `type_name_of_val(&7u32 as &dyn Debug)` 可能返回 `"dyn Debug"`,而不会返回
/// `"u32"`。
///
/// # 示例
///
/// 打印默认的整数和浮点类型。
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

/// 如果 `T` 能被强转成 trait 对象类型 `U`,返回 `Some(&U)`;否则返回 `None`。
///
/// # 编译期失败
/// 判定 `T` 能否被强转成 trait 对象类型 `U` 需要编译器进行 trait 解析。
/// 在某些情况下,该解析可能超出递归上限,届时编译会失败,而不是让本函数
/// 返回 `None`。
/// # 示例
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
/// struct Rock; // 没有实现 Animal
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
            // SAFETY:`t` 是指向某个类型的引用,所以我们知道它是有效的。
            // `dyn_metadata` 是 T 的一份虚表,实现了 `U` 所代表的 trait。
            Some(unsafe { &*pointer })
        }
        None => None,
    }
}

/// 如果 `T` 能被强转成 trait 对象类型 `U`,返回 `Some(&mut U)`;否则返回 `None`。
///
/// # 编译期失败
/// 判定 `T` 能否被强转成 trait 对象类型 `U` 需要编译器进行 trait 解析。
/// 在某些情况下,该解析可能超出递归上限,届时编译会失败,而不是让本函数
/// 返回 `None`。
/// # 示例
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
/// struct Rock; // 没有实现 Animal
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
            // SAFETY:`t` 是指向某个类型的引用,所以我们知道它是有效的。
            // `dyn_metadata` 是 T 的一份虚表,实现了 `U` 所代表的 trait。
            Some(unsafe { &mut *pointer })
        }
        None => None,
    }
}
